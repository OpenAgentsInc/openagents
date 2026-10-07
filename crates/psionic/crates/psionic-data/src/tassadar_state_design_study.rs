use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_STATE_DESIGN_STUDY_ABI_VERSION: &str = "psionic.tassadar.state_design_study.v1";
pub const TASSADAR_STATE_DESIGN_STUDY_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/state_design_study";
pub const TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_state_design_runtime_report.json";
pub const TASSADAR_STATE_DESIGN_STUDY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_state_design_study_report.json";

/// One same-workload comparison row inside the public state-design study contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignStudyRow {
    /// Stable workload-family label.
    pub workload_family: String,
    /// Compared design-family labels.
    pub compared_design_families: Vec<String>,
    /// Evaluation axes applied to this workload family.
    pub evaluation_axes: Vec<String>,
    /// Existing artifact refs anchoring this workload family.
    pub anchor_refs: Vec<String>,
    /// Plain-language workload note.
    pub note: String,
}

impl TassadarStateDesignStudyRow {
    fn new(
        workload_family: &str,
        compared_design_families: &[&str],
        anchor_refs: &[&str],
        note: &str,
    ) -> Self {
        Self {
            workload_family: String::from(workload_family),
            compared_design_families: compared_design_families
                .iter()
                .map(|family| String::from(*family))
                .collect(),
            evaluation_axes: vec![
                String::from("locality_score_bps"),
                String::from("edit_cost_bps"),
                String::from("replayability"),
                String::from("exact_output_preservation"),
                String::from("refusal_threshold"),
            ],
            anchor_refs: anchor_refs
                .iter()
                .map(|reference| String::from(*reference))
                .collect(),
            note: String::from(note),
        }
    }
}

/// Public data contract for the state-design study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStateDesignStudyContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable contract reference.
    pub contract_ref: String,
    /// Stable version label.
    pub version: String,
    /// Compared design-family labels.
    pub compared_design_families: Vec<String>,
    /// Same-workload study rows.
    pub workload_rows: Vec<TassadarStateDesignStudyRow>,
    /// Runtime artifact produced by the study.
    pub runtime_report_ref: String,
    /// Eval artifact produced by the study.
    pub study_report_ref: String,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

impl TassadarStateDesignStudyContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_STATE_DESIGN_STUDY_ABI_VERSION),
            contract_ref: String::from(TASSADAR_STATE_DESIGN_STUDY_CONTRACT_REF),
            version: String::from("2026.03.18"),
            compared_design_families: vec![
                String::from("full_append_only_trace"),
                String::from("delta_trace"),
                String::from("locality_scratchpad"),
                String::from("recurrent_state"),
                String::from("working_memory_tier"),
            ],
            workload_rows: vec![
                TassadarStateDesignStudyRow::new(
                    "module_call_trace",
                    &[
                        "full_append_only_trace",
                        "delta_trace",
                        "locality_scratchpad",
                    ],
                    &[
                        "fixtures/tassadar/reports/tassadar_module_trace_abi_v2_report.json",
                        "fixtures/tassadar/reports/tassadar_trace_abi_decision_report.json",
                    ],
                    "module-call trace workloads test whether state compression still preserves auditability across frame boundaries instead of only final outputs",
                ),
                TassadarStateDesignStudyRow::new(
                    "symbolic_locality",
                    &[
                        "full_append_only_trace",
                        "delta_trace",
                        "locality_scratchpad",
                    ],
                    &[
                        "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json",
                        "fixtures/tassadar/reports/tassadar_scratchpad_framework_comparison_report.json",
                    ],
                    "symbolic-locality workloads isolate positional span and edit-cost changes without changing the underlying symbolic truth",
                ),
                TassadarStateDesignStudyRow::new(
                    "associative_recall",
                    &[
                        "full_append_only_trace",
                        "recurrent_state",
                        "working_memory_tier",
                    ],
                    &[
                        "fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json",
                        "fixtures/tassadar/reports/tassadar_working_memory_tier_eval_report.json",
                    ],
                    "associative-recall workloads test when explicit memory state beats append-only trace expansion",
                ),
                TassadarStateDesignStudyRow::new(
                    "long_horizon_control",
                    &[
                        "full_append_only_trace",
                        "recurrent_state",
                        "working_memory_tier",
                    ],
                    &[
                        "fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json",
                        "fixtures/tassadar/reports/tassadar_approximate_attention_closure_matrix.json",
                    ],
                    "long-horizon control workloads test whether carried state survives when exact replay becomes too expensive",
                ),
                TassadarStateDesignStudyRow::new(
                    "byte_memory_loop",
                    &[
                        "full_append_only_trace",
                        "delta_trace",
                        "locality_scratchpad",
                        "recurrent_state",
                        "working_memory_tier",
                    ],
                    &[
                        "fixtures/tassadar/reports/tassadar_memory_abi_v2_report.json",
                        "fixtures/tassadar/reports/tassadar_working_memory_tier_runtime_report.json",
                        "fixtures/tassadar/reports/tassadar_recurrent_fast_path_runtime_baseline.json",
                    ],
                    "byte-memory loop workloads stress byte-addressed mutation, replay reconstruction, and bounded state publication on the same semantic family",
                ),
            ],
            runtime_report_ref: String::from(TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF),
            study_report_ref: String::from(TASSADAR_STATE_DESIGN_STUDY_REPORT_REF),
            claim_boundary: String::from(
                "this contract defines one same-workload representation study over trace, delta, scratchpad, recurrent, and working-memory designs. It keeps replayability, exactness, and refusal boundaries explicit instead of treating one encoding win as broad executor closure",
            ),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("state-design study contract should validate");
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_state_design_study_contract|", &contract);
        contract
    }

    /// Validates the public study contract.
    pub fn validate(&self) -> Result<(), TassadarStateDesignStudyContractError> {
        if self.abi_version != TASSADAR_STATE_DESIGN_STUDY_ABI_VERSION {
            return Err(
                TassadarStateDesignStudyContractError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarStateDesignStudyContractError::MissingContractRef);
        }
        if self.compared_design_families.is_empty() {
            return Err(TassadarStateDesignStudyContractError::MissingDesignFamilies);
        }
        if self.workload_rows.is_empty() {
            return Err(TassadarStateDesignStudyContractError::MissingWorkloads);
        }
        for row in &self.workload_rows {
            if row.workload_family.trim().is_empty() {
                return Err(TassadarStateDesignStudyContractError::MissingWorkloadFamily);
            }
            if row.compared_design_families.is_empty() {
                return Err(
                    TassadarStateDesignStudyContractError::MissingRowDesignFamilies {
                        workload_family: row.workload_family.clone(),
                    },
                );
            }
            if row.anchor_refs.is_empty() {
                return Err(TassadarStateDesignStudyContractError::MissingAnchorRefs {
                    workload_family: row.workload_family.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Validation failure for the public study contract.
#[derive(Debug, Error)]
pub enum TassadarStateDesignStudyContractError {
    #[error("unsupported ABI version `{abi_version}` for state-design study contract")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("state-design study contract is missing `contract_ref`")]
    MissingContractRef,
    #[error("state-design study contract is missing compared design families")]
    MissingDesignFamilies,
    #[error("state-design study contract is missing workload rows")]
    MissingWorkloads,
    #[error("state-design study contract contains an empty workload family")]
    MissingWorkloadFamily,
    #[error("state-design study row `{workload_family}` is missing compared design families")]
    MissingRowDesignFamilies { workload_family: String },
    #[error("state-design study row `{workload_family}` is missing anchor refs")]
    MissingAnchorRefs { workload_family: String },
}

/// Returns the canonical public contract for the state-design study.
#[must_use]
pub fn tassadar_state_design_study_contract() -> TassadarStateDesignStudyContract {
    TassadarStateDesignStudyContract::new()
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
        TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF, TASSADAR_STATE_DESIGN_STUDY_CONTRACT_REF,
        tassadar_state_design_study_contract,
    };

    #[test]
    fn state_design_study_contract_is_machine_legible() {
        let contract = tassadar_state_design_study_contract();

        assert_eq!(
            contract.contract_ref,
            TASSADAR_STATE_DESIGN_STUDY_CONTRACT_REF
        );
        assert_eq!(contract.workload_rows.len(), 5);
        assert_eq!(
            contract.runtime_report_ref,
            TASSADAR_STATE_DESIGN_RUNTIME_REPORT_REF
        );
        assert!(!contract.contract_digest.is_empty());
    }
}
