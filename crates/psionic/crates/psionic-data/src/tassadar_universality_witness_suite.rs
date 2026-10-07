use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_UNIVERSALITY_WITNESS_SUITE_ABI_VERSION: &str =
    "psionic.tassadar.universality_witness_suite.v1";
pub const TASSADAR_UNIVERSALITY_WITNESS_SUITE_REF: &str =
    "dataset://openagents/tassadar/universality_witness_suite";
pub const TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_universality_witness_suite_report.json";
pub const TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_universality_witness_suite_summary.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarUniversalityWitnessFamily {
    RegisterMachine,
    TapeMachine,
    BytecodeVmInterpreter,
    SessionProcessKernel,
    SpillTapeContinuation,
    BytecodeVmParamBoundary,
    ExternalEventLoopBoundary,
}

impl TassadarUniversalityWitnessFamily {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RegisterMachine => "register_machine",
            Self::TapeMachine => "tape_machine",
            Self::BytecodeVmInterpreter => "bytecode_vm_interpreter",
            Self::SessionProcessKernel => "session_process_kernel",
            Self::SpillTapeContinuation => "spill_tape_continuation",
            Self::BytecodeVmParamBoundary => "bytecode_vm_param_boundary",
            Self::ExternalEventLoopBoundary => "external_event_loop_boundary",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarUniversalityWitnessExpectation {
    Exact,
    RefusalBoundary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityWitnessRow {
    pub witness_family: TassadarUniversalityWitnessFamily,
    pub expected_status: TassadarUniversalityWitnessExpectation,
    pub authority_refs: Vec<String>,
    pub claim_boundary: String,
}

impl TassadarUniversalityWitnessRow {
    fn validate(&self) -> Result<(), TassadarUniversalityWitnessSuiteError> {
        if self.authority_refs.is_empty() {
            return Err(
                TassadarUniversalityWitnessSuiteError::MissingAuthorityRefs {
                    witness_family: self.witness_family,
                },
            );
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(
                TassadarUniversalityWitnessSuiteError::MissingClaimBoundary {
                    witness_family: self.witness_family,
                },
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarUniversalityWitnessSuiteContract {
    pub abi_version: String,
    pub suite_ref: String,
    pub version: String,
    pub witness_rows: Vec<TassadarUniversalityWitnessRow>,
    pub evaluation_axes: Vec<String>,
    pub report_ref: String,
    pub summary_report_ref: String,
    pub contract_digest: String,
}

impl TassadarUniversalityWitnessSuiteContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_ABI_VERSION),
            suite_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REF),
            version: String::from("2026.03.19"),
            witness_rows: witness_rows(),
            evaluation_axes: vec![
                String::from("exact_runtime_parity"),
                String::from("checkpoint_resume_equivalent"),
                String::from("refusal_boundary_held"),
                String::from("runtime_envelope"),
                String::from("evidence_anchor"),
            ],
            report_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("universality witness suite contract should validate");
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_universality_witness_suite_contract|",
            &contract,
        );
        contract
    }

    pub fn validate(&self) -> Result<(), TassadarUniversalityWitnessSuiteError> {
        if self.abi_version != TASSADAR_UNIVERSALITY_WITNESS_SUITE_ABI_VERSION {
            return Err(
                TassadarUniversalityWitnessSuiteError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.suite_ref.trim().is_empty() {
            return Err(TassadarUniversalityWitnessSuiteError::MissingSuiteRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarUniversalityWitnessSuiteError::MissingVersion);
        }
        if self.witness_rows.is_empty() {
            return Err(TassadarUniversalityWitnessSuiteError::MissingWitnessRows);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarUniversalityWitnessSuiteError::MissingEvaluationAxes);
        }
        if self.report_ref.trim().is_empty() || self.summary_report_ref.trim().is_empty() {
            return Err(TassadarUniversalityWitnessSuiteError::MissingReportRefs);
        }

        let mut seen_families = BTreeSet::new();
        for row in &self.witness_rows {
            row.validate()?;
            if !seen_families.insert(row.witness_family) {
                return Err(
                    TassadarUniversalityWitnessSuiteError::DuplicateWitnessFamily {
                        witness_family: row.witness_family,
                    },
                );
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn family_ids_by_expectation(
        &self,
        expected_status: TassadarUniversalityWitnessExpectation,
    ) -> Vec<TassadarUniversalityWitnessFamily> {
        self.witness_rows
            .iter()
            .filter(|row| row.expected_status == expected_status)
            .map(|row| row.witness_family)
            .collect()
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarUniversalityWitnessSuiteError {
    #[error("unsupported universality-witness-suite ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("universality-witness-suite contract is missing `suite_ref`")]
    MissingSuiteRef,
    #[error("universality-witness-suite contract is missing `version`")]
    MissingVersion,
    #[error("universality-witness-suite contract is missing `witness_rows`")]
    MissingWitnessRows,
    #[error(
        "universality-witness-suite contract has duplicate witness family `{witness_family:?}`"
    )]
    DuplicateWitnessFamily {
        witness_family: TassadarUniversalityWitnessFamily,
    },
    #[error(
        "universality-witness-suite contract is missing authority refs for `{witness_family:?}`"
    )]
    MissingAuthorityRefs {
        witness_family: TassadarUniversalityWitnessFamily,
    },
    #[error(
        "universality-witness-suite contract is missing claim boundary for `{witness_family:?}`"
    )]
    MissingClaimBoundary {
        witness_family: TassadarUniversalityWitnessFamily,
    },
    #[error("universality-witness-suite contract is missing `evaluation_axes`")]
    MissingEvaluationAxes,
    #[error("universality-witness-suite contract is missing report refs")]
    MissingReportRefs,
}

#[must_use]
pub fn tassadar_universality_witness_suite_contract() -> TassadarUniversalityWitnessSuiteContract {
    TassadarUniversalityWitnessSuiteContract::new()
}

fn witness_rows() -> Vec<TassadarUniversalityWitnessRow> {
    vec![
        row(
            TassadarUniversalityWitnessFamily::RegisterMachine,
            TassadarUniversalityWitnessExpectation::Exact,
            &["fixtures/tassadar/reports/tassadar_universal_machine_proof_report.json"],
            "register-machine universality stays bounded to the committed two-register witness over `TCM.v1` rather than implying broader arbitrary-machine closure",
        ),
        row(
            TassadarUniversalityWitnessFamily::TapeMachine,
            TassadarUniversalityWitnessExpectation::Exact,
            &["fixtures/tassadar/reports/tassadar_universal_machine_proof_report.json"],
            "tape-machine universality stays bounded to the committed single-tape witness over `TCM.v1` rather than implying unbounded semantic-window widening",
        ),
        row(
            TassadarUniversalityWitnessFamily::BytecodeVmInterpreter,
            TassadarUniversalityWitnessExpectation::Exact,
            &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "bytecode-vm interpretation stays bounded to the current deterministic vm-style module family and its explicit parameter-ABI refusal sibling rather than implying arbitrary Wasm or arbitrary interpreter closure",
        ),
        row(
            TassadarUniversalityWitnessFamily::SessionProcessKernel,
            TassadarUniversalityWitnessExpectation::Exact,
            &["fixtures/tassadar/reports/tassadar_session_process_profile_report.json"],
            "session-process kernels stay bounded to deterministic echo and counter-turn loops rather than implying open-ended external event processing or generic agent loops",
        ),
        row(
            TassadarUniversalityWitnessFamily::SpillTapeContinuation,
            TassadarUniversalityWitnessExpectation::Exact,
            &["fixtures/tassadar/reports/tassadar_spill_tape_store_report.json"],
            "spill/tape continuation stays bounded to the current-host cpu-reference envelope and explicit segment-store artifacts rather than implying arbitrary persistent memory or async effect closure",
        ),
        row(
            TassadarUniversalityWitnessFamily::BytecodeVmParamBoundary,
            TassadarUniversalityWitnessExpectation::RefusalBoundary,
            &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "vm-style parameter ABI mismatch remains an explicit refusal boundary inside the witness suite rather than a silent widening to arbitrary call signatures",
        ),
        row(
            TassadarUniversalityWitnessFamily::ExternalEventLoopBoundary,
            TassadarUniversalityWitnessExpectation::RefusalBoundary,
            &["fixtures/tassadar/reports/tassadar_session_process_profile_report.json"],
            "open-ended external event loops remain an explicit refusal boundary inside the witness suite rather than a silent widening to generic long-running agent processes",
        ),
    ]
}

fn row(
    witness_family: TassadarUniversalityWitnessFamily,
    expected_status: TassadarUniversalityWitnessExpectation,
    authority_refs: &[&str],
    claim_boundary: &str,
) -> TassadarUniversalityWitnessRow {
    TassadarUniversalityWitnessRow {
        witness_family,
        expected_status,
        authority_refs: authority_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        claim_boundary: String::from(claim_boundary),
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
        tassadar_universality_witness_suite_contract, TassadarUniversalityWitnessExpectation,
        TassadarUniversalityWitnessFamily, TASSADAR_UNIVERSALITY_WITNESS_SUITE_ABI_VERSION,
        TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF,
        TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF,
    };

    #[test]
    fn universality_witness_suite_contract_keeps_exact_and_refusal_families_explicit() {
        let contract = tassadar_universality_witness_suite_contract();

        assert_eq!(
            contract.abi_version,
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_ABI_VERSION
        );
        assert_eq!(contract.witness_rows.len(), 7);
        assert_eq!(
            contract.family_ids_by_expectation(TassadarUniversalityWitnessExpectation::Exact),
            vec![
                TassadarUniversalityWitnessFamily::RegisterMachine,
                TassadarUniversalityWitnessFamily::TapeMachine,
                TassadarUniversalityWitnessFamily::BytecodeVmInterpreter,
                TassadarUniversalityWitnessFamily::SessionProcessKernel,
                TassadarUniversalityWitnessFamily::SpillTapeContinuation,
            ]
        );
        assert_eq!(
            contract
                .family_ids_by_expectation(TassadarUniversalityWitnessExpectation::RefusalBoundary),
            vec![
                TassadarUniversalityWitnessFamily::BytecodeVmParamBoundary,
                TassadarUniversalityWitnessFamily::ExternalEventLoopBoundary,
            ]
        );
        assert_eq!(
            contract.report_ref,
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF
        );
        assert_eq!(
            contract.summary_report_ref,
            TASSADAR_UNIVERSALITY_WITNESS_SUITE_SUMMARY_REF
        );
    }
}
