use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::TassadarTrapExceptionExpectedTerminalKind;
use psionic_ir::TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID;

const CONTRACT_SCHEMA_VERSION: u16 = 1;

/// One public compiler-owned case specification for the exceptions profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileCaseSpec {
    pub case_id: String,
    pub semantic_id: String,
    pub workload_family: String,
    pub expected_terminal_kind: TassadarTrapExceptionExpectedTerminalKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_non_success_kind: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Public compiler-owned contract for the exceptions profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExceptionProfileCompilationContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub profile_id: String,
    pub claim_class: String,
    pub case_specs: Vec<TassadarExceptionProfileCaseSpec>,
    pub claim_boundary: String,
    pub summary: String,
    pub contract_digest: String,
}

impl TassadarExceptionProfileCompilationContract {
    fn new(case_specs: Vec<TassadarExceptionProfileCaseSpec>) -> Self {
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
            contract_id: String::from("tassadar.exception_profile.compilation_contract.v1"),
            profile_id: String::from(TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID),
            claim_class: String::from(
                "compiled_bounded_exactness / execution_truth / refusal_truth",
            ),
            case_specs,
            claim_boundary: String::from(
                "this contract freezes one bounded exceptions proposal profile over typed throw, catch, and rethrow semantics. It keeps malformed handlers, delegate, try_table, and broader exception-family widening on explicit refusal paths instead of inheriting them from frozen core-Wasm or generic trap parity",
            ),
            summary: String::new(),
            contract_digest: String::new(),
        };
        contract.summary = format!(
            "Exception profile compilation contract freezes {} cases across {} success, {} trap, and {} refusal expectations.",
            contract.case_specs.len(),
            success_case_count,
            trap_case_count,
            refusal_case_count,
        );
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_exception_profile_compilation_contract|",
            &contract,
        );
        contract
    }
}

/// Returns the canonical compiler-owned exception-profile contract.
#[must_use]
pub fn compile_tassadar_exception_profile_contract() -> TassadarExceptionProfileCompilationContract
{
    TassadarExceptionProfileCompilationContract::new(vec![
        case_spec(
            "throw_catch_success",
            "throw_catch_success",
            "module_scale_exception_dispatch",
            TassadarTrapExceptionExpectedTerminalKind::Success,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_trap_exception_report.json",
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
            ],
            "typed throw and catch should recover to an exact success state instead of degrading into a generic trap",
        ),
        case_spec(
            "nested_rethrow_trap_stack",
            "nested_rethrow_trap_stack",
            "search_exception_unwind",
            TassadarTrapExceptionExpectedTerminalKind::Trap,
            Some("uncaught_rethrow"),
            &[
                "fixtures/tassadar/reports/tassadar_trap_exception_report.json",
                "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
            ],
            "nested rethrow should preserve typed trap-stack shape instead of collapsing into an undifferentiated failure",
        ),
        case_spec(
            "handler_tag_mismatch_trap",
            "handler_tag_mismatch_trap",
            "exception_handler_dispatch",
            TassadarTrapExceptionExpectedTerminalKind::Trap,
            Some("handler_tag_mismatch"),
            &[
                "fixtures/tassadar/reports/tassadar_trap_exception_report.json",
                "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
            ],
            "tag mismatches should remain trap-parity cases rather than looking like malformed-module refusals",
        ),
        case_spec(
            "malformed_exception_handler_refusal",
            "malformed_exception_handler_refusal",
            "exception_handler_boundary",
            TassadarTrapExceptionExpectedTerminalKind::Refusal,
            Some("malformed_exception_handler"),
            &["fixtures/tassadar/reports/tassadar_trap_exception_report.json"],
            "malformed exception handlers should stay explicit refusal truth before execution planning starts",
        ),
    ])
}

fn case_spec(
    case_id: &str,
    semantic_id: &str,
    workload_family: &str,
    expected_terminal_kind: TassadarTrapExceptionExpectedTerminalKind,
    expected_non_success_kind: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarExceptionProfileCaseSpec {
    TassadarExceptionProfileCaseSpec {
        case_id: String::from(case_id),
        semantic_id: String::from(semantic_id),
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
    use super::compile_tassadar_exception_profile_contract;
    use crate::TassadarTrapExceptionExpectedTerminalKind;

    #[test]
    fn exception_profile_compilation_contract_is_machine_legible() {
        let contract = compile_tassadar_exception_profile_contract();

        assert_eq!(contract.case_specs.len(), 4);
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_terminal_kind == TassadarTrapExceptionExpectedTerminalKind::Trap
                && case.expected_non_success_kind.as_deref() == Some("uncaught_rethrow")
        }));
        assert!(contract.case_specs.iter().any(|case| {
            case.expected_terminal_kind == TassadarTrapExceptionExpectedTerminalKind::Refusal
                && case.expected_non_success_kind.as_deref() == Some("malformed_exception_handler")
        }));
        assert!(!contract.contract_digest.is_empty());
    }
}
