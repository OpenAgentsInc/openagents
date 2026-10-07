use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CONTRACT_SCHEMA_VERSION: u16 = 1;

/// Expected terminal kind for one trap/exception semantics case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTrapExceptionExpectedTerminalKind {
    Success,
    Trap,
    Refusal,
}

impl TassadarTrapExceptionExpectedTerminalKind {
    /// Returns the stable label for the terminal kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Trap => "trap",
            Self::Refusal => "refusal",
        }
    }
}

/// One public compiler-owned case specification for trap/exception closure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrapExceptionCaseSpec {
    pub case_id: String,
    pub workload_family: String,
    pub expected_terminal_kind: TassadarTrapExceptionExpectedTerminalKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_non_success_kind: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Public compiler-owned contract for trap/exception semantics closure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTrapExceptionContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub claim_class: String,
    pub case_specs: Vec<TassadarTrapExceptionCaseSpec>,
    pub claim_boundary: String,
    pub summary: String,
    pub contract_digest: String,
}

impl TassadarTrapExceptionContract {
    fn new(case_specs: Vec<TassadarTrapExceptionCaseSpec>) -> Self {
        let success_case_count = case_specs
            .iter()
            .filter(|case| {
                case.expected_terminal_kind == TassadarTrapExceptionExpectedTerminalKind::Success
            })
            .count();
        let trap_case_count = case_specs
            .iter()
            .filter(|case| {
                case.expected_terminal_kind == TassadarTrapExceptionExpectedTerminalKind::Trap
            })
            .count();
        let refusal_case_count = case_specs
            .iter()
            .filter(|case| {
                case.expected_terminal_kind == TassadarTrapExceptionExpectedTerminalKind::Refusal
            })
            .count();
        let mut contract = Self {
            schema_version: CONTRACT_SCHEMA_VERSION,
            contract_id: String::from("tassadar.trap_exception.contract.v1"),
            claim_class: String::from(
                "execution_truth / compiled_bounded_exactness / refusal_truth",
            ),
            case_specs,
            claim_boundary: String::from(
                "this contract is a benchmark-bound execution-truth surface over success, trap, and refusal cases in the widened Wasm lane. It keeps bounds faults, indirect-call failures, malformed imports, and unsupported-profile refusals explicit instead of letting successful exactness stand in for failure-path closure",
            ),
            summary: String::new(),
            contract_digest: String::new(),
        };
        contract.summary = format!(
            "Trap/exception contract freezes {} cases across {} success, {} trap, and {} refusal expectations.",
            contract.case_specs.len(),
            success_case_count,
            trap_case_count,
            refusal_case_count,
        );
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_trap_exception_contract|", &contract);
        contract
    }
}

/// Returns the canonical compiler-owned trap/exception contract.
#[must_use]
pub fn compile_tassadar_trap_exception_contract() -> TassadarTrapExceptionContract {
    TassadarTrapExceptionContract::new(vec![
        case_spec(
            "arithmetic_reference_success",
            "arithmetic_multi_operand",
            TassadarTrapExceptionExpectedTerminalKind::Success,
            None,
            &[
                "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_exactness_report.json",
            ],
            "seeded arithmetic case where success parity remains the control row for the trap/exception audit",
        ),
        case_spec(
            "module_scale_bounds_fault",
            "module_scale_wasm_loop",
            TassadarTrapExceptionExpectedTerminalKind::Trap,
            Some("bounds_fault"),
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
                "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
            ],
            "module-scale Wasm case where byte-addressed memory overflow must trap with explicit state parity",
        ),
        case_spec(
            "sudoku_indirect_call_failure",
            "sudoku_backtracking_search",
            TassadarTrapExceptionExpectedTerminalKind::Trap,
            Some("indirect_call_failure"),
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
            ],
            "search-heavy case where indirect-call target or signature failure must remain challengeable instead of collapsing into generic search loss",
        ),
        case_spec(
            "malformed_import_refusal",
            "malformed_import_boundary",
            TassadarTrapExceptionExpectedTerminalKind::Refusal,
            Some("malformed_import"),
            &["fixtures/tassadar/reports/tassadar_wasm_conformance_report.json"],
            "malformed import surface where refusal truth must remain explicit before execution starts",
        ),
        case_spec(
            "unsupported_profile_refusal",
            "clrs_shortest_path",
            TassadarTrapExceptionExpectedTerminalKind::Refusal,
            Some("unsupported_profile_refusal"),
            &[
                "fixtures/tassadar/reports/tassadar_exactness_refusal_report.json",
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            ],
            "unsupported profile request where refusal parity should stay as visible as successful execution parity",
        ),
    ])
}

fn case_spec(
    case_id: &str,
    workload_family: &str,
    expected_terminal_kind: TassadarTrapExceptionExpectedTerminalKind,
    expected_non_success_kind: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarTrapExceptionCaseSpec {
    TassadarTrapExceptionCaseSpec {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        expected_terminal_kind,
        expected_non_success_kind: expected_non_success_kind.map(String::from),
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
        TassadarTrapExceptionExpectedTerminalKind, compile_tassadar_trap_exception_contract,
    };

    #[test]
    fn trap_exception_contract_is_machine_legible() {
        let contract = compile_tassadar_trap_exception_contract();

        assert_eq!(contract.case_specs.len(), 5);
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_terminal_kind == TassadarTrapExceptionExpectedTerminalKind::Trap
                && case.expected_non_success_kind.as_deref() == Some("bounds_fault")
        }));
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_terminal_kind == TassadarTrapExceptionExpectedTerminalKind::Refusal
                && case.expected_non_success_kind.as_deref() == Some("malformed_import")
        }));
        assert!(!contract.contract_digest.is_empty());
    }
}
