use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_PROGRAM_FAMILY_FRONTIER_ABI_VERSION: &str =
    "psionic.tassadar.program_family_frontier.v1";
pub const TASSADAR_PROGRAM_FAMILY_FRONTIER_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/program_family_frontier";
pub const TASSADAR_PROGRAM_FAMILY_FRONTIER_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_program_family_frontier_v1/program_family_frontier_bundle.json";
pub const TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_program_family_frontier_report.json";
pub const TASSADAR_PROGRAM_FAMILY_FRONTIER_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_program_family_frontier_summary.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarProgramFamilyArchitectureFamily {
    CompiledExactReference,
    LearnedStructuredMemory,
    VerifierAttachedHybrid,
}

impl TassadarProgramFamilyArchitectureFamily {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompiledExactReference => "compiled_exact_reference",
            Self::LearnedStructuredMemory => "learned_structured_memory",
            Self::VerifierAttachedHybrid => "verifier_attached_hybrid",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarProgramFamilyGeneralizationSplit {
    InFamily,
    HeldOutFamily,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarProgramFamilyWorkloadFamily {
    KernelStateMachine,
    SearchProcessMachine,
    LinkedProgramBundle,
    EffectfulResumeGraph,
    MultiModulePackageWorkflow,
    HeldOutVirtualMachine,
    HeldOutMessageOrchestrator,
}

impl TassadarProgramFamilyWorkloadFamily {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KernelStateMachine => "kernel_state_machine",
            Self::SearchProcessMachine => "search_process_machine",
            Self::LinkedProgramBundle => "linked_program_bundle",
            Self::EffectfulResumeGraph => "effectful_resume_graph",
            Self::MultiModulePackageWorkflow => "multi_module_package_workflow",
            Self::HeldOutVirtualMachine => "held_out_virtual_machine",
            Self::HeldOutMessageOrchestrator => "held_out_message_orchestrator",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyFrontierWorkloadRow {
    pub workload_family: TassadarProgramFamilyWorkloadFamily,
    pub split: TassadarProgramFamilyGeneralizationSplit,
    pub compared_architectures: Vec<TassadarProgramFamilyArchitectureFamily>,
    pub authority_refs: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyFrontierContract {
    pub abi_version: String,
    pub contract_ref: String,
    pub version: String,
    pub architecture_families: Vec<TassadarProgramFamilyArchitectureFamily>,
    pub workload_rows: Vec<TassadarProgramFamilyFrontierWorkloadRow>,
    pub evaluation_axes: Vec<String>,
    pub evidence_bundle_ref: String,
    pub report_ref: String,
    pub summary_report_ref: String,
    pub contract_digest: String,
}

impl TassadarProgramFamilyFrontierContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_PROGRAM_FAMILY_FRONTIER_ABI_VERSION),
            contract_ref: String::from(TASSADAR_PROGRAM_FAMILY_FRONTIER_CONTRACT_REF),
            version: String::from("2026.03.19"),
            architecture_families: vec![
                TassadarProgramFamilyArchitectureFamily::CompiledExactReference,
                TassadarProgramFamilyArchitectureFamily::LearnedStructuredMemory,
                TassadarProgramFamilyArchitectureFamily::VerifierAttachedHybrid,
            ],
            workload_rows: workload_rows(),
            evaluation_axes: vec![
                String::from("later_window_exactness_bps"),
                String::from("final_output_exactness_bps"),
                String::from("refusal_calibration_bps"),
                String::from("normalized_cost_units"),
                String::from("held_out_family_frontier_rank"),
            ],
            evidence_bundle_ref: String::from(TASSADAR_PROGRAM_FAMILY_FRONTIER_BUNDLE_REF),
            report_ref: String::from(TASSADAR_PROGRAM_FAMILY_FRONTIER_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_PROGRAM_FAMILY_FRONTIER_SUMMARY_REF),
            contract_digest: String::new(),
        };
        contract.validate().expect("contract should validate");
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_program_family_frontier_contract|",
            &contract,
        );
        contract
    }

    pub fn validate(&self) -> Result<(), TassadarProgramFamilyFrontierContractError> {
        if self.abi_version != TASSADAR_PROGRAM_FAMILY_FRONTIER_ABI_VERSION {
            return Err(
                TassadarProgramFamilyFrontierContractError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarProgramFamilyFrontierContractError::MissingContractRef);
        }
        if self.architecture_families.is_empty() {
            return Err(TassadarProgramFamilyFrontierContractError::MissingArchitectureFamilies);
        }
        if self.workload_rows.is_empty() {
            return Err(TassadarProgramFamilyFrontierContractError::MissingWorkloadRows);
        }
        let mut seen_architectures = BTreeSet::new();
        for architecture in &self.architecture_families {
            if !seen_architectures.insert(*architecture) {
                return Err(
                    TassadarProgramFamilyFrontierContractError::DuplicateArchitectureFamily {
                        architecture_family: *architecture,
                    },
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyFrontierEvidenceCase {
    pub case_id: String,
    pub architecture_family: TassadarProgramFamilyArchitectureFamily,
    pub workload_family: TassadarProgramFamilyWorkloadFamily,
    pub split: TassadarProgramFamilyGeneralizationSplit,
    pub later_window_exactness_bps: u32,
    pub final_output_exactness_bps: u32,
    pub refusal_calibration_bps: u32,
    pub normalized_cost_units: u32,
    pub dominant_failure_mode: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramFamilyFrontierBundle {
    pub contract: TassadarProgramFamilyFrontierContract,
    pub case_reports: Vec<TassadarProgramFamilyFrontierEvidenceCase>,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarProgramFamilyFrontierContractError {
    #[error("unsupported ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("missing contract ref")]
    MissingContractRef,
    #[error("missing architecture families")]
    MissingArchitectureFamilies,
    #[error("missing workload rows")]
    MissingWorkloadRows,
    #[error("duplicate architecture family `{architecture_family:?}`")]
    DuplicateArchitectureFamily {
        architecture_family: TassadarProgramFamilyArchitectureFamily,
    },
}

#[must_use]
pub fn tassadar_program_family_frontier_contract() -> TassadarProgramFamilyFrontierContract {
    TassadarProgramFamilyFrontierContract::new()
}

fn workload_rows() -> Vec<TassadarProgramFamilyFrontierWorkloadRow> {
    let architectures = vec![
        TassadarProgramFamilyArchitectureFamily::CompiledExactReference,
        TassadarProgramFamilyArchitectureFamily::LearnedStructuredMemory,
        TassadarProgramFamilyArchitectureFamily::VerifierAttachedHybrid,
    ];
    vec![
        row(
            TassadarProgramFamilyWorkloadFamily::KernelStateMachine,
            TassadarProgramFamilyGeneralizationSplit::InFamily,
            architectures.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_call_frame_report.json",
                "fixtures/tassadar/reports/tassadar_workload_capability_frontier_report.json",
            ],
            "kernel-state rows remain a bounded family-transfer anchor and do not imply arbitrary low-level program closure",
        ),
        row(
            TassadarProgramFamilyWorkloadFamily::SearchProcessMachine,
            TassadarProgramFamilyGeneralizationSplit::InFamily,
            architectures.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_search_native_executor_report.json",
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
            ],
            "search-process rows remain bounded to verifier-shaped seeded workloads rather than broad open-ended search ownership",
        ),
        row(
            TassadarProgramFamilyWorkloadFamily::LinkedProgramBundle,
            TassadarProgramFamilyGeneralizationSplit::InFamily,
            architectures.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_linked_program_bundle_eval_report.json",
                "fixtures/tassadar/reports/tassadar_cross_profile_link_eval_report.json",
            ],
            "linked-program rows remain bounded to the named internal-compute bundle graphs and do not imply arbitrary linking closure",
        ),
        row(
            TassadarProgramFamilyWorkloadFamily::EffectfulResumeGraph,
            TassadarProgramFamilyGeneralizationSplit::InFamily,
            architectures.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json",
                "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json",
            ],
            "effectful-resume rows stay inside the bounded replay and lifecycle envelope and do not widen effect-safe execution by inheritance",
        ),
        row(
            TassadarProgramFamilyWorkloadFamily::MultiModulePackageWorkflow,
            TassadarProgramFamilyGeneralizationSplit::InFamily,
            architectures.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_internal_compute_package_manager_eval_report.json",
                "fixtures/tassadar/reports/tassadar_linked_program_bundle_eval_report.json",
            ],
            "multi-module workflow rows remain tied to the explicit package-manager and linked-program benchmark families rather than arbitrary package ecosystems",
        ),
        row(
            TassadarProgramFamilyWorkloadFamily::HeldOutVirtualMachine,
            TassadarProgramFamilyGeneralizationSplit::HeldOutFamily,
            architectures.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_learned_call_stack_heap_suite_report.json",
                "fixtures/tassadar/reports/tassadar_search_native_executor_report.json",
            ],
            "held-out virtual-machine rows measure cross-family generalization only and do not widen any learned or hybrid lane into broad virtual-machine ownership",
        ),
        row(
            TassadarProgramFamilyWorkloadFamily::HeldOutMessageOrchestrator,
            TassadarProgramFamilyGeneralizationSplit::HeldOutFamily,
            architectures,
            &[
                "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json",
                "fixtures/tassadar/reports/tassadar_cross_profile_link_eval_report.json",
            ],
            "held-out message-orchestrator rows remain an explicit held-out ladder and do not imply general interactive process closure",
        ),
    ]
}

fn row(
    workload_family: TassadarProgramFamilyWorkloadFamily,
    split: TassadarProgramFamilyGeneralizationSplit,
    compared_architectures: Vec<TassadarProgramFamilyArchitectureFamily>,
    authority_refs: &[&str],
    claim_boundary: &str,
) -> TassadarProgramFamilyFrontierWorkloadRow {
    TassadarProgramFamilyFrontierWorkloadRow {
        workload_family,
        split,
        compared_architectures,
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
        tassadar_program_family_frontier_contract, TassadarProgramFamilyArchitectureFamily,
        TassadarProgramFamilyGeneralizationSplit, TassadarProgramFamilyWorkloadFamily,
    };

    #[test]
    fn program_family_frontier_contract_is_machine_legible() {
        let contract = tassadar_program_family_frontier_contract();

        assert_eq!(
            contract.contract_ref,
            "dataset://openagents/tassadar/program_family_frontier"
        );
        assert_eq!(contract.architecture_families.len(), 3);
        assert!(!contract.contract_digest.is_empty());
    }

    #[test]
    fn program_family_frontier_contract_keeps_held_out_rows_explicit() {
        let contract = tassadar_program_family_frontier_contract();

        let held_out_row = contract
            .workload_rows
            .iter()
            .find(|row| {
                row.workload_family == TassadarProgramFamilyWorkloadFamily::HeldOutVirtualMachine
            })
            .expect("held-out row");
        assert_eq!(
            held_out_row.split,
            TassadarProgramFamilyGeneralizationSplit::HeldOutFamily
        );
        assert!(held_out_row
            .compared_architectures
            .contains(&TassadarProgramFamilyArchitectureFamily::VerifierAttachedHybrid));
    }
}
