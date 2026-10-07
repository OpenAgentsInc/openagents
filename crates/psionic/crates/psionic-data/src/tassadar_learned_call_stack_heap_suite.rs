use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_ABI_VERSION: &str =
    "psionic.tassadar.learned_call_stack_heap_suite.v1";
pub const TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/learned_call_stack_heap_suite";
pub const TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_learned_call_stack_heap_suite_v1/learned_call_stack_heap_suite_bundle.json";
pub const TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_learned_call_stack_heap_suite_report.json";
pub const TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_learned_call_stack_heap_suite_summary.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCallStackHeapModelVariant {
    BaselineTransformer,
    StructuredMemory,
}

impl TassadarCallStackHeapModelVariant {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaselineTransformer => "baseline_transformer",
            Self::StructuredMemory => "structured_memory",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCallStackHeapGeneralizationSplit {
    InFamily,
    HeldOutFamily,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCallStackHeapWorkloadFamily {
    RecursiveEvaluator,
    ParserFrameMachine,
    BumpAllocatorHeap,
    FreeListAllocatorHeap,
    ResumableProcessHeap,
    HeldOutContinuationMachine,
    HeldOutAllocatorScheduler,
}

impl TassadarCallStackHeapWorkloadFamily {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RecursiveEvaluator => "recursive_evaluator",
            Self::ParserFrameMachine => "parser_frame_machine",
            Self::BumpAllocatorHeap => "bump_allocator_heap",
            Self::FreeListAllocatorHeap => "free_list_allocator_heap",
            Self::ResumableProcessHeap => "resumable_process_heap",
            Self::HeldOutContinuationMachine => "held_out_continuation_machine",
            Self::HeldOutAllocatorScheduler => "held_out_allocator_scheduler",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapWorkloadRow {
    pub workload_family: TassadarCallStackHeapWorkloadFamily,
    pub split: TassadarCallStackHeapGeneralizationSplit,
    pub compared_model_variants: Vec<TassadarCallStackHeapModelVariant>,
    pub authority_refs: Vec<String>,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapSuiteContract {
    pub abi_version: String,
    pub contract_ref: String,
    pub version: String,
    pub model_variants: Vec<TassadarCallStackHeapModelVariant>,
    pub workload_rows: Vec<TassadarLearnedCallStackHeapWorkloadRow>,
    pub evaluation_axes: Vec<String>,
    pub evidence_bundle_ref: String,
    pub report_ref: String,
    pub summary_report_ref: String,
    pub contract_digest: String,
}

impl TassadarLearnedCallStackHeapSuiteContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_ABI_VERSION),
            contract_ref: String::from(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_CONTRACT_REF),
            version: String::from("2026.03.19"),
            model_variants: vec![
                TassadarCallStackHeapModelVariant::BaselineTransformer,
                TassadarCallStackHeapModelVariant::StructuredMemory,
            ],
            workload_rows: workload_rows(),
            evaluation_axes: vec![
                String::from("later_window_exactness_bps"),
                String::from("final_output_exactness_bps"),
                String::from("refusal_calibration_bps"),
                String::from("max_call_depth"),
                String::from("max_heap_cells"),
            ],
            evidence_bundle_ref: String::from(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_BUNDLE_REF),
            report_ref: String::from(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_SUMMARY_REF),
            contract_digest: String::new(),
        };
        contract.validate().expect("contract should validate");
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_learned_call_stack_heap_suite_contract|",
            &contract,
        );
        contract
    }

    pub fn validate(&self) -> Result<(), TassadarLearnedCallStackHeapSuiteContractError> {
        if self.abi_version != TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_ABI_VERSION {
            return Err(
                TassadarLearnedCallStackHeapSuiteContractError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarLearnedCallStackHeapSuiteContractError::MissingContractRef);
        }
        if self.model_variants.is_empty() {
            return Err(TassadarLearnedCallStackHeapSuiteContractError::MissingModelVariants);
        }
        if self.workload_rows.is_empty() {
            return Err(TassadarLearnedCallStackHeapSuiteContractError::MissingWorkloads);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapEvidenceCase {
    pub case_id: String,
    pub model_variant: TassadarCallStackHeapModelVariant,
    pub workload_family: TassadarCallStackHeapWorkloadFamily,
    pub split: TassadarCallStackHeapGeneralizationSplit,
    pub later_window_exactness_bps: u32,
    pub final_output_exactness_bps: u32,
    pub refusal_calibration_bps: u32,
    pub max_call_depth: u32,
    pub max_heap_cells: u32,
    pub dominant_failure_mode: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedCallStackHeapSuiteBundle {
    pub contract: TassadarLearnedCallStackHeapSuiteContract,
    pub case_reports: Vec<TassadarLearnedCallStackHeapEvidenceCase>,
    pub summary: String,
    pub report_digest: String,
}

fn workload_rows() -> Vec<TassadarLearnedCallStackHeapWorkloadRow> {
    let variants = vec![
        TassadarCallStackHeapModelVariant::BaselineTransformer,
        TassadarCallStackHeapModelVariant::StructuredMemory,
    ];
    vec![
        row(
            TassadarCallStackHeapWorkloadFamily::RecursiveEvaluator,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            variants.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_call_frame_report.json",
                "fixtures/tassadar/reports/tassadar_learnability_gap_report.json",
            ],
            "recursive evaluator coverage remains a bounded learned-stack benchmark and does not imply general recursion closure",
        ),
        row(
            TassadarCallStackHeapWorkloadFamily::ParserFrameMachine,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            variants.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_call_frame_report.json",
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
            ],
            "parser-frame coverage remains bounded to the learned benchmark suite rather than broad parser execution closure",
        ),
        row(
            TassadarCallStackHeapWorkloadFamily::BumpAllocatorHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            variants.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
            ],
            "bump-allocator coverage remains a bounded heap benchmark and does not imply arbitrary allocator semantics",
        ),
        row(
            TassadarCallStackHeapWorkloadFamily::FreeListAllocatorHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            variants.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
                "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json",
            ],
            "free-list coverage remains bounded to the explicit heap benchmark family and does not widen portability or process claims",
        ),
        row(
            TassadarCallStackHeapWorkloadFamily::ResumableProcessHeap,
            TassadarCallStackHeapGeneralizationSplit::InFamily,
            variants.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_process_object_report.json",
                "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json",
            ],
            "resumable-process heap coverage remains bounded to seeded process-style benchmarks rather than general interactive process closure",
        ),
        row(
            TassadarCallStackHeapWorkloadFamily::HeldOutContinuationMachine,
            TassadarCallStackHeapGeneralizationSplit::HeldOutFamily,
            variants.clone(),
            &[
                "fixtures/tassadar/reports/tassadar_search_native_executor_report.json",
                "fixtures/tassadar/reports/tassadar_learnability_gap_report.json",
            ],
            "held-out continuation-machine coverage measures generalization only and does not promote the learned lane into broad process ownership",
        ),
        row(
            TassadarCallStackHeapWorkloadFamily::HeldOutAllocatorScheduler,
            TassadarCallStackHeapGeneralizationSplit::HeldOutFamily,
            variants,
            &[
                "fixtures/tassadar/reports/tassadar_search_native_executor_report.json",
                "fixtures/tassadar/reports/tassadar_installed_process_lifecycle_report.json",
            ],
            "held-out allocator-scheduler coverage remains a bounded generalization check with explicit refusal sensitivity",
        ),
    ]
}

fn row(
    workload_family: TassadarCallStackHeapWorkloadFamily,
    split: TassadarCallStackHeapGeneralizationSplit,
    compared_model_variants: Vec<TassadarCallStackHeapModelVariant>,
    authority_refs: &[&str],
    claim_boundary: &str,
) -> TassadarLearnedCallStackHeapWorkloadRow {
    TassadarLearnedCallStackHeapWorkloadRow {
        workload_family,
        split,
        compared_model_variants,
        authority_refs: authority_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        claim_boundary: String::from(claim_boundary),
    }
}

pub fn tassadar_learned_call_stack_heap_suite_contract() -> TassadarLearnedCallStackHeapSuiteContract
{
    TassadarLearnedCallStackHeapSuiteContract::new()
}

#[derive(Debug, Error)]
pub enum TassadarLearnedCallStackHeapSuiteContractError {
    #[error("unsupported abi version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("missing contract ref")]
    MissingContractRef,
    #[error("missing model variants")]
    MissingModelVariants,
    #[error("missing workloads")]
    MissingWorkloads,
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
        TassadarCallStackHeapGeneralizationSplit, TassadarCallStackHeapWorkloadFamily,
        tassadar_learned_call_stack_heap_suite_contract,
    };

    #[test]
    fn learned_call_stack_heap_contract_is_machine_legible() {
        let contract = tassadar_learned_call_stack_heap_suite_contract();

        assert_eq!(contract.model_variants.len(), 2);
        assert_eq!(contract.workload_rows.len(), 7);
        assert!(contract.workload_rows.iter().any(|row| {
            row.workload_family == TassadarCallStackHeapWorkloadFamily::HeldOutContinuationMachine
                && row.split == TassadarCallStackHeapGeneralizationSplit::HeldOutFamily
        }));
        assert!(!contract.contract_digest.is_empty());
    }
}
