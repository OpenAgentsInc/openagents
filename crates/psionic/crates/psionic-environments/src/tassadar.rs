use std::collections::BTreeMap;

use psionic_data::{
    DatasetKey, TassadarBenchmarkAxis, TassadarBenchmarkFamily, TassadarClrsAlgorithmFamily,
    TassadarClrsLengthBucket, TassadarClrsTrajectoryFamily, TassadarModuleScaleWorkloadFamily,
    tassadar_broad_program_family_suite_contract,
};
use psionic_models::{
    TassadarGeneralizedAbiPublication, TassadarInternalComputeProfileClaimCheckResult,
    TassadarInternalComputeProfileLadderPublication,
    TassadarRustArticleProfileCompletenessPublication,
    check_tassadar_internal_compute_profile_claim,
    tassadar_current_served_internal_compute_profile_claim, tassadar_generalized_abi_publication,
    tassadar_internal_compute_profile_ladder_publication,
    tassadar_rust_article_profile_completeness_publication,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use thiserror::Error;

use crate::{
    EnvironmentBenchmarkProfile, EnvironmentCompositionGroup, EnvironmentCompositionMember,
    EnvironmentContractError, EnvironmentDatasetBinding, EnvironmentExecutionEntrypoint,
    EnvironmentGroupResolution, EnvironmentPackageContract, EnvironmentPackageFamily,
    EnvironmentPackageInstallSource, EnvironmentPackageKey, EnvironmentPolicyKind,
    EnvironmentPolicyReference, EnvironmentRegistry, EnvironmentRegistryError,
    EnvironmentRubricHook, EnvironmentRuntimeFamily, EnvironmentStateMode, EnvironmentUsageSurface,
    EnvironmentVerificationPosture, EnvironmentWorkloadClass,
};

const TASSADAR_METADATA_SURFACE_KEY: &str = "tassadar.surface";
const TASSADAR_METADATA_PACKAGE_REFS_KEY: &str = "tassadar.package_refs";
const TASSADAR_METADATA_PROGRAM_BINDING_KEY: &str = "tassadar.program_binding";
const TASSADAR_METADATA_IO_CONTRACT_KEY: &str = "tassadar.io_contract";
const TASSADAR_METADATA_EXACTNESS_CONTRACT_KEY: &str = "tassadar.exactness_contract";
const TASSADAR_METADATA_CURRENT_TARGETS_KEY: &str = "tassadar.current_workload_targets";
const TASSADAR_METADATA_PLANNED_TARGETS_KEY: &str = "tassadar.planned_workload_targets";
const TASSADAR_METADATA_BENCHMARK_PACKAGE_SET_KEY: &str = "tassadar.benchmark_package_set";
const TASSADAR_METADATA_COMPILE_PIPELINE_MATRIX_KEY: &str = "tassadar.compile_pipeline_matrix";
const TASSADAR_METADATA_RUST_ARTICLE_PROFILE_KEY: &str = "tassadar.rust_article_profile";
const TASSADAR_METADATA_GENERALIZED_ABI_FAMILY_KEY: &str = "tassadar.generalized_abi_family";
const TASSADAR_METADATA_EXECUTION_CHECKPOINT_KEY: &str = "tassadar.execution_checkpoint";
const TASSADAR_METADATA_PROCESS_OBJECT_KEY: &str = "tassadar.process_object_family";
const TASSADAR_METADATA_SPILL_TAPE_STORE_KEY: &str = "tassadar.spill_tape_store_profile";
const TASSADAR_METADATA_DYNAMIC_MEMORY_RESUME_KEY: &str = "tassadar.dynamic_memory_resume";
const TASSADAR_METADATA_SIMULATOR_EFFECT_PROFILE_KEY: &str = "tassadar.simulator_effect_profile";
const TASSADAR_METADATA_MEMORY64_PROFILE_KEY: &str = "tassadar.memory64_profile";
const TASSADAR_METADATA_INTERNAL_COMPUTE_PROFILE_LADDER_KEY: &str =
    "tassadar.internal_compute_profile_ladder";
const TASSADAR_METADATA_INTERNAL_COMPUTE_PROFILE_CLAIM_KEY: &str =
    "tassadar.internal_compute_profile_claim";
const TASSADAR_METADATA_BROAD_INTERNAL_COMPUTE_PORTABILITY_KEY: &str =
    "tassadar.broad_internal_compute_portability";
const TASSADAR_METADATA_NUMERIC_PORTABILITY_KEY: &str = "tassadar.numeric_portability";
const TASSADAR_METADATA_FLOAT_SEMANTICS_KEY: &str = "tassadar.float_semantics";
const TASSADAR_METADATA_WASM_CONFORMANCE_KEY: &str = "tassadar.wasm_conformance";
const TASSADAR_METADATA_ARCHITECTURE_BAKEOFF_KEY: &str = "tassadar.architecture_bakeoff";
const TASSADAR_METADATA_MODULE_SCALE_WORKLOAD_SUITE_KEY: &str =
    "tassadar.module_scale_workload_suite";
const TASSADAR_METADATA_CLRS_WASM_BRIDGE_KEY: &str = "tassadar.clrs_wasm_bridge";
const TASSADAR_METADATA_ABI_VERSION_KEY: &str = "tassadar.abi_version";
const TASSADAR_EVAL_METADATA_SURFACE: &str = "eval";
const TASSADAR_BENCHMARK_METADATA_SURFACE: &str = "benchmark";

/// Stable ABI version for the typed Tassadar environment bundle helper.
pub const TASSADAR_ENVIRONMENT_ABI_VERSION: &str = "psionic.tassadar_environment.v1";

/// Explicit workload-target taxonomy for the Tassadar benchmark corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkloadTarget {
    /// Arithmetic-only microprograms.
    ArithmeticMicroprogram,
    /// CLRS-adjacent shortest-path witness workloads.
    ClrsShortestPath,
    /// Memory or local read-write microprograms.
    MemoryLookupMicroprogram,
    /// Branch and control-flow microprograms.
    BranchControlFlowMicroprogram,
    /// Richer WebAssembly kernels beyond the current microprogram corpus.
    MicroWasmKernel,
    /// Branch-heavy kernel programs with repeated control-flow pivots.
    BranchHeavyKernel,
    /// Memory-heavy kernel programs with dense read/write traffic.
    MemoryHeavyKernel,
    /// Long-loop kernels that push the executor toward longer horizons.
    LongLoopKernel,
    /// Sudoku-style exact search workloads.
    SudokuClass,
    /// Hungarian or min-cost-matching style workloads.
    HungarianMatching,
    /// Fixed-span module-scale memcpy-style workloads.
    ModuleMemcpy,
    /// Fixed-token module-scale parsing workloads.
    ModuleParsing,
    /// Fixed-span module-scale checksum workloads.
    ModuleChecksum,
    /// Multi-export module-scale VM-style dispatch workloads.
    ModuleVmStyle,
}

/// Stable package refs that the Tassadar eval and benchmark surfaces reuse.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarEnvironmentPackageRefs {
    /// Stable environment group reference.
    pub group_ref: String,
    /// Pin alias for the eval package.
    pub eval_pin_alias: String,
    /// Pin alias for the benchmark package.
    pub benchmark_pin_alias: String,
    /// Member ref for the eval package.
    pub eval_member_ref: String,
    /// Member ref for the benchmark package.
    pub benchmark_member_ref: String,
    /// Stable program-corpus reference.
    pub program_corpus_ref: String,
    /// Stable IO-contract reference.
    pub io_contract_ref: String,
    /// Stable rubric-binding reference.
    pub rubric_binding_ref: String,
    /// Stable runtime-profile ref for eval execution.
    pub eval_runtime_profile_ref: String,
    /// Stable benchmark profile ref.
    pub benchmark_profile_ref: String,
    /// Stable runtime-profile ref for benchmark execution.
    pub benchmark_runtime_profile_ref: String,
}

/// Public benchmark-package-set binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBenchmarkPackageSetBinding {
    /// Stable benchmark-package-set reference.
    pub package_set_ref: String,
    /// Immutable package-set version.
    pub package_set_version: String,
    /// Benchmark families surfaced by this environment bundle.
    pub supported_families: Vec<TassadarBenchmarkFamily>,
    /// Reporting axes surfaced by this environment bundle.
    pub axis_coverage: Vec<TassadarBenchmarkAxis>,
    /// Canonical machine-readable summary artifact ref for the package set.
    pub summary_report_ref: String,
}

impl TassadarBenchmarkPackageSetBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar benchmark package-set binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.package_set_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkPackageSetRef);
        }
        if self.package_set_version.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkPackageSetVersion);
        }
        if self.supported_families.is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkFamilies);
        }
        if self.axis_coverage.is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkAxisCoverage);
        }
        if self.summary_report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkSummaryReportRef);
        }
        Ok(())
    }
}

/// Public compile-pipeline matrix binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompilePipelineMatrixBinding {
    /// Stable compile-pipeline report reference.
    pub report_ref: String,
    /// Stable compile-pipeline report identifier.
    pub report_id: String,
    /// Source families covered by the committed report.
    pub source_family_ids: Vec<String>,
}

impl TassadarCompilePipelineMatrixBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar compile-pipeline matrix binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingCompilePipelineMatrixReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingCompilePipelineMatrixReportId);
        }
        if self.source_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingCompilePipelineSourceFamilies);
        }
        if self
            .source_family_ids
            .iter()
            .any(|source_family_id| source_family_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidCompilePipelineSourceFamilyId);
        }
        Ok(())
    }
}

/// Public checkpointed multi-slice execution binding reused by Tassadar
/// environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionCheckpointBinding {
    /// Stable execution-checkpoint report reference.
    pub report_ref: String,
    /// Stable execution-checkpoint report identifier.
    pub report_id: String,
    /// Stable run-bundle reference carrying persisted continuation artifacts.
    pub run_bundle_ref: String,
    /// Stable checkpoint-family identifier.
    pub checkpoint_family_id: String,
    /// Workload families covered by the committed report.
    pub workload_family_ids: Vec<String>,
}

impl TassadarExecutionCheckpointBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar execution-checkpoint binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingExecutionCheckpointReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingExecutionCheckpointReportId);
        }
        if self.run_bundle_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingExecutionCheckpointRunBundleRef);
        }
        if self.checkpoint_family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingExecutionCheckpointFamilyId);
        }
        if self.workload_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingExecutionCheckpointWorkloadFamilies);
        }
        if self
            .workload_family_ids
            .iter()
            .any(|workload_family_id| workload_family_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidExecutionCheckpointWorkloadFamilyId);
        }
        Ok(())
    }
}

/// Returns the canonical checkpointed multi-slice execution binding reused by
/// Tassadar environment surfaces.
#[must_use]
pub fn default_tassadar_execution_checkpoint_binding() -> TassadarExecutionCheckpointBinding {
    TassadarExecutionCheckpointBinding {
        report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
        ),
        report_id: String::from("tassadar.execution_checkpoint.report.v1"),
        run_bundle_ref: String::from(
            "fixtures/tassadar/runs/tassadar_execution_checkpoint_v1/tassadar_execution_checkpoint_bundle.json",
        ),
        checkpoint_family_id: String::from("tassadar.execution_checkpoint.v1"),
        workload_family_ids: vec![
            String::from("long_loop_kernel"),
            String::from("state_machine_accumulator"),
            String::from("search_frontier_kernel"),
        ],
    }
}

/// Public durable process snapshot, tape, and work-queue binding reused by
/// Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProcessObjectBinding {
    /// Stable durable-process report reference.
    pub report_ref: String,
    /// Stable durable-process report identifier.
    pub report_id: String,
    /// Stable run-bundle reference carrying persisted process objects.
    pub run_bundle_ref: String,
    /// Stable process-snapshot family identifier.
    pub snapshot_family_id: String,
    /// Stable process-tape family identifier.
    pub tape_family_id: String,
    /// Stable process work-queue family identifier.
    pub work_queue_family_id: String,
    /// Stable process identifiers covered by the committed report.
    pub process_ids: Vec<String>,
}

impl TassadarProcessObjectBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("Tassadar process-object binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingProcessObjectReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingProcessObjectReportId);
        }
        if self.run_bundle_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingProcessObjectRunBundleRef);
        }
        if self.snapshot_family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingProcessSnapshotFamilyId);
        }
        if self.tape_family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingProcessTapeFamilyId);
        }
        if self.work_queue_family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingProcessWorkQueueFamilyId);
        }
        if self.process_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingProcessObjectProcessIds);
        }
        if self
            .process_ids
            .iter()
            .any(|process_id| process_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidProcessObjectProcessId);
        }
        Ok(())
    }
}

/// Returns the canonical durable process-object binding reused by Tassadar
/// environment surfaces.
#[must_use]
pub fn default_tassadar_process_object_binding() -> TassadarProcessObjectBinding {
    TassadarProcessObjectBinding {
        report_ref: String::from("fixtures/tassadar/reports/tassadar_process_object_report.json"),
        report_id: String::from("tassadar.process_object.report.v1"),
        run_bundle_ref: String::from(
            "fixtures/tassadar/runs/tassadar_process_objects_v1/tassadar_process_object_bundle.json",
        ),
        snapshot_family_id: String::from("tassadar.process_snapshot.v1"),
        tape_family_id: String::from("tassadar.process_tape.v1"),
        work_queue_family_id: String::from("tassadar.process_work_queue.v1"),
        process_ids: vec![
            String::from("tassadar.process.long_loop_kernel.v1"),
            String::from("tassadar.process.state_machine_accumulator.v1"),
            String::from("tassadar.process.search_frontier_kernel.v1"),
        ],
    }
}

/// Public spill-aware continuation binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSpillTapeStoreBinding {
    /// Stable spill/tape report reference.
    pub report_ref: String,
    /// Stable spill/tape report identifier.
    pub report_id: String,
    /// Stable run-bundle reference carrying persisted spill/tape artifacts.
    pub run_bundle_ref: String,
    /// Stable named spill/tape profile identifier.
    pub profile_id: String,
    /// Stable spill-segment family identifier.
    pub spill_segment_family_id: String,
    /// Stable external tape-store family identifier.
    pub external_tape_store_family_id: String,
    /// Stable portability envelope identifiers carried by the committed report.
    pub portability_envelope_ids: Vec<String>,
    /// Stable case identifiers covered by the committed report.
    pub case_ids: Vec<String>,
}

impl TassadarSpillTapeStoreBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("Tassadar spill/tape binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSpillTapeStoreReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSpillTapeStoreReportId);
        }
        if self.run_bundle_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSpillTapeStoreRunBundleRef);
        }
        if self.profile_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSpillTapeStoreProfileId);
        }
        if self.spill_segment_family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSpillSegmentFamilyId);
        }
        if self.external_tape_store_family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingExternalTapeStoreFamilyId);
        }
        if self.portability_envelope_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingSpillTapeStorePortabilityEnvelopes);
        }
        if self
            .portability_envelope_ids
            .iter()
            .any(|envelope_id| envelope_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidSpillTapeStorePortabilityEnvelopeId);
        }
        if self.case_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingSpillTapeStoreCaseIds);
        }
        if self.case_ids.iter().any(|case_id| case_id.trim().is_empty()) {
            return Err(TassadarEnvironmentError::InvalidSpillTapeStoreCaseId);
        }
        Ok(())
    }
}

/// Returns the canonical spill-aware continuation binding reused by Tassadar environment surfaces.
#[must_use]
pub fn default_tassadar_spill_tape_store_binding() -> TassadarSpillTapeStoreBinding {
    TassadarSpillTapeStoreBinding {
        report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
        ),
        report_id: String::from("tassadar.spill_tape_store.report.v1"),
        run_bundle_ref: String::from(
            "fixtures/tassadar/runs/tassadar_spill_tape_store_v1/tassadar_spill_tape_store_bundle.json",
        ),
        profile_id: String::from("tassadar.internal_compute.spill_tape_store.v1"),
        spill_segment_family_id: String::from("tassadar.spill_segment.v1"),
        external_tape_store_family_id: String::from("tassadar.external_tape_store.v1"),
        portability_envelope_ids: vec![String::from("cpu_reference_current_host")],
        case_ids: vec![
            String::from("long_loop_spill_resume"),
            String::from("search_frontier_spill_resume"),
            String::from("state_machine_oversize_spill_refusal"),
        ],
    }
}

/// Public dynamic-memory pause-and-resume binding reused by Tassadar
/// environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDynamicMemoryResumeBinding {
    /// Stable dynamic-memory resume report reference.
    pub report_ref: String,
    /// Stable dynamic-memory resume report identifier.
    pub report_id: String,
    /// Stable run-bundle reference carrying persisted checkpoints.
    pub run_bundle_ref: String,
    /// Stable checkpoint-family identifier.
    pub checkpoint_family_id: String,
    /// Case identifiers covered by the committed report.
    pub case_ids: Vec<String>,
}

impl TassadarDynamicMemoryResumeBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar dynamic-memory resume binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingDynamicMemoryResumeReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingDynamicMemoryResumeReportId);
        }
        if self.run_bundle_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingDynamicMemoryResumeRunBundleRef);
        }
        if self.checkpoint_family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingDynamicMemoryResumeFamilyId);
        }
        if self.case_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingDynamicMemoryResumeCaseIds);
        }
        if self
            .case_ids
            .iter()
            .any(|case_id| case_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidDynamicMemoryResumeCaseId);
        }
        Ok(())
    }
}

/// Returns the canonical dynamic-memory pause-and-resume binding reused by
/// Tassadar environment surfaces.
#[must_use]
pub fn default_tassadar_dynamic_memory_resume_binding() -> TassadarDynamicMemoryResumeBinding {
    TassadarDynamicMemoryResumeBinding {
        report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
        ),
        report_id: String::from("tassadar.dynamic_memory_resume.report.v1"),
        run_bundle_ref: String::from(
            "fixtures/tassadar/runs/tassadar_dynamic_memory_resume_v1/tassadar_dynamic_memory_resume_bundle.json",
        ),
        checkpoint_family_id: String::from("tassadar.dynamic_memory_resume.v1"),
        case_ids: vec![String::from("copy_fill_pause_after_copy")],
    }
}

/// Public deterministic simulator-backed effect binding reused by Tassadar
/// environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorEffectProfileBinding {
    /// Stable simulator-effect profile report reference.
    pub report_ref: String,
    /// Stable simulator-effect profile report identifier.
    pub report_id: String,
    /// Stable run-bundle reference carrying the seeded simulator receipts.
    pub run_bundle_ref: String,
    /// Stable named profile identifier.
    pub profile_id: String,
    /// Named simulator profiles admitted by the bounded effect surface.
    pub simulator_profile_ids: Vec<String>,
    /// Portability envelopes covered by the committed report.
    pub portability_envelope_ids: Vec<String>,
    /// Ambient effect ids kept on explicit refusal paths.
    pub refused_effect_ids: Vec<String>,
    /// Case identifiers covered by the committed report.
    pub case_ids: Vec<String>,
}

impl TassadarSimulatorEffectProfileBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar simulator-effect profile binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSimulatorEffectProfileReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSimulatorEffectProfileReportId);
        }
        if self.run_bundle_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSimulatorEffectProfileRunBundleRef);
        }
        if self.profile_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingSimulatorEffectProfileId);
        }
        if self.simulator_profile_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingSimulatorEffectProfileIds);
        }
        if self
            .simulator_profile_ids
            .iter()
            .any(|profile_id| profile_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidSimulatorEffectProfileId);
        }
        if self.portability_envelope_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingSimulatorEffectPortabilityEnvelopes);
        }
        if self
            .portability_envelope_ids
            .iter()
            .any(|envelope_id| envelope_id.trim().is_empty())
        {
            return Err(
                TassadarEnvironmentError::InvalidSimulatorEffectPortabilityEnvelopeId,
            );
        }
        if self.refused_effect_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingSimulatorEffectRefusedEffectIds);
        }
        if self
            .refused_effect_ids
            .iter()
            .any(|effect_id| effect_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidSimulatorEffectRefusedEffectId);
        }
        if self.case_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingSimulatorEffectCaseIds);
        }
        if self.case_ids.iter().any(|case_id| case_id.trim().is_empty()) {
            return Err(TassadarEnvironmentError::InvalidSimulatorEffectCaseId);
        }
        Ok(())
    }
}

/// Returns the canonical deterministic simulator-backed effect binding reused
/// by Tassadar environment surfaces.
#[must_use]
pub fn default_tassadar_simulator_effect_profile_binding() -> TassadarSimulatorEffectProfileBinding
{
    TassadarSimulatorEffectProfileBinding {
        report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_simulator_effect_profile_report.json",
        ),
        report_id: String::from("tassadar.simulator_effect.profile_report.v1"),
        run_bundle_ref: String::from(
            "fixtures/tassadar/runs/tassadar_simulator_effects_v1/tassadar_simulator_effect_runtime_bundle.json",
        ),
        profile_id: String::from("tassadar.effect_profile.simulator_backed_io.v1"),
        simulator_profile_ids: vec![
            String::from("sim.clock.seeded_epoch_250ms.v1"),
            String::from("sim.random.pcg64_seed_42.v1"),
            String::from("sim.network.loopback_fifo_seed_7.v1"),
        ],
        portability_envelope_ids: vec![String::from("cpu_reference_current_host")],
        refused_effect_ids: vec![
            String::from("host.clock.now"),
            String::from("host.random.os_entropy"),
            String::from("host.network.socket_io"),
        ],
        case_ids: vec![
            String::from("seeded_clock_tick_case"),
            String::from("seeded_randomness_stream_case"),
            String::from("seeded_network_loopback_case"),
            String::from("ambient_system_clock_refusal"),
            String::from("ambient_os_random_refusal"),
            String::from("ambient_socket_network_refusal"),
        ],
    }
}

/// Public bounded memory64 continuation binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemory64ProfileBinding {
    /// Stable memory64 profile report reference.
    pub report_ref: String,
    /// Stable memory64 profile report identifier.
    pub report_id: String,
    /// Stable run-bundle reference carrying persisted sparse checkpoints.
    pub run_bundle_ref: String,
    /// Stable checkpoint-family identifier.
    pub checkpoint_family_id: String,
    /// Stable named profile identifier.
    pub profile_id: String,
    /// Stable portability envelope identifier.
    pub portability_envelope_id: String,
    /// Maximum virtual-address width admitted by the bounded profile.
    pub max_virtual_address_bits: u8,
    /// Case identifiers covered by the committed report.
    pub case_ids: Vec<String>,
}

impl TassadarMemory64ProfileBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("Tassadar memory64 profile binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingMemory64ProfileReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingMemory64ProfileReportId);
        }
        if self.run_bundle_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingMemory64ProfileRunBundleRef);
        }
        if self.checkpoint_family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingMemory64ProfileFamilyId);
        }
        if self.profile_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingMemory64ProfileId);
        }
        if self.portability_envelope_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingMemory64PortabilityEnvelopeId);
        }
        if self.max_virtual_address_bits < 64 {
            return Err(TassadarEnvironmentError::InvalidMemory64ProfileAddressBits);
        }
        if self.case_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingMemory64ProfileCaseIds);
        }
        if self
            .case_ids
            .iter()
            .any(|case_id| case_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidMemory64ProfileCaseId);
        }
        Ok(())
    }
}

/// Returns the canonical bounded memory64 continuation binding reused by Tassadar environment surfaces.
#[must_use]
pub fn default_tassadar_memory64_profile_binding() -> TassadarMemory64ProfileBinding {
    TassadarMemory64ProfileBinding {
        report_ref: String::from("fixtures/tassadar/reports/tassadar_memory64_profile_report.json"),
        report_id: String::from("tassadar.memory64_profile.report.v1"),
        run_bundle_ref: String::from(
            "fixtures/tassadar/runs/tassadar_memory64_resume_v1/tassadar_memory64_resume_bundle.json",
        ),
        checkpoint_family_id: String::from("tassadar.memory64_resume.v1"),
        profile_id: String::from("tassadar.proposal_profile.memory64_continuation.v1"),
        portability_envelope_id: String::from("cpu_reference_current_host"),
        max_virtual_address_bits: 64,
        case_ids: vec![
            String::from("sparse_above_4g_resume"),
            String::from("memory_grow_above_4g_resume"),
            String::from("backend_virtual_address_limit_refusal"),
        ],
    }
}

/// Public architecture-bakeoff binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArchitectureBakeoffBinding {
    /// Stable suite reference.
    pub suite_ref: String,
    /// Immutable suite version.
    pub suite_version: String,
    /// Workload families covered by the broadened matrix.
    pub workload_family_ids: Vec<String>,
    /// Stable architecture bakeoff report reference.
    pub report_ref: String,
    /// Stable architecture bakeoff summary reference.
    pub summary_report_ref: String,
}

impl TassadarArchitectureBakeoffBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar architecture bakeoff binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.suite_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingArchitectureBakeoffSuiteRef);
        }
        if self.suite_version.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingArchitectureBakeoffSuiteVersion);
        }
        if self.workload_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingArchitectureBakeoffWorkloadFamilies);
        }
        if self
            .workload_family_ids
            .iter()
            .any(|workload_family_id| workload_family_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidArchitectureBakeoffWorkloadFamilyId);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingArchitectureBakeoffReportRef);
        }
        if self.summary_report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingArchitectureBakeoffSummaryReportRef);
        }
        Ok(())
    }
}

/// Returns the canonical architecture-bakeoff binding reused by Tassadar
/// environment surfaces.
#[must_use]
pub fn default_tassadar_architecture_bakeoff_binding() -> TassadarArchitectureBakeoffBinding {
    let suite = tassadar_broad_program_family_suite_contract();
    let workload_family_ids = suite.workload_family_ids();
    TassadarArchitectureBakeoffBinding {
        suite_ref: suite.suite_ref,
        suite_version: suite.version,
        workload_family_ids,
        report_ref: suite.report_ref,
        summary_report_ref: suite.summary_report_ref,
    }
}

/// Public portability and acceptance-gate binding reused by Tassadar
/// environment bundles for broader internal-compute publication control.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputePortabilityBinding {
    /// Stable portability report reference.
    pub report_ref: String,
    /// Stable portability report identifier.
    pub report_id: String,
    /// Stable broad acceptance-gate report reference.
    pub acceptance_gate_ref: String,
    /// Stable broad acceptance-gate report identifier.
    pub acceptance_gate_id: String,
    /// Backend families carried by the committed portability matrix.
    pub backend_family_ids: Vec<String>,
    /// Toolchain families carried by the committed portability matrix.
    pub toolchain_family_ids: Vec<String>,
    /// Named profiles carried by the binding.
    pub profile_ids: Vec<String>,
}

impl TassadarBroadInternalComputePortabilityBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar broad internal-compute portability binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBroadInternalComputePortabilityReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBroadInternalComputePortabilityReportId);
        }
        if self.acceptance_gate_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBroadInternalComputeAcceptanceGateRef);
        }
        if self.acceptance_gate_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBroadInternalComputeAcceptanceGateId);
        }
        if self.backend_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingBroadInternalComputeBackendFamilyIds);
        }
        if self
            .backend_family_ids
            .iter()
            .any(|backend_family_id| backend_family_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidBroadInternalComputeBackendFamilyId);
        }
        if self.toolchain_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingBroadInternalComputeToolchainFamilyIds);
        }
        if self
            .toolchain_family_ids
            .iter()
            .any(|toolchain_family_id| toolchain_family_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidBroadInternalComputeToolchainFamilyId);
        }
        if self.profile_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingBroadInternalComputeProfileIds);
        }
        if self
            .profile_ids
            .iter()
            .any(|profile_id| profile_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidBroadInternalComputeProfileId);
        }
        Ok(())
    }
}

/// Returns the canonical broader internal-compute portability binding reused by
/// Tassadar environment surfaces.
#[must_use]
pub fn default_tassadar_broad_internal_compute_portability_binding()
-> TassadarBroadInternalComputePortabilityBinding {
    TassadarBroadInternalComputePortabilityBinding {
        report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json",
        ),
        report_id: String::from("tassadar.broad_internal_compute_portability.report.v1"),
        acceptance_gate_ref: String::from(
            "fixtures/tassadar/reports/tassadar_broad_internal_compute_acceptance_gate.json",
        ),
        acceptance_gate_id: String::from(
            "tassadar.broad_internal_compute_acceptance_gate.report.v1",
        ),
        backend_family_ids: vec![
            String::from("cpu_reference"),
            String::from("cuda_served"),
            String::from("metal_served"),
        ],
        toolchain_family_ids: vec![
            String::from("rustc:wasm32-unknown-unknown"),
            String::from("rustc:wasm32-unknown-unknown+cuda_served"),
            String::from("rustc:wasm32-unknown-unknown+metal_served"),
        ],
        profile_ids: vec![
            String::from("tassadar.internal_compute.article_closeout.v1"),
            String::from("tassadar.internal_compute.generalized_abi.v1"),
            String::from("tassadar.internal_compute.runtime_support_subset.v1"),
            String::from("tassadar.internal_compute.deterministic_import_subset.v1"),
            String::from("tassadar.internal_compute.portable_broad_family.v1"),
            String::from("tassadar.internal_compute.public_broad_family.v1"),
        ],
    }
}

/// Public numeric-portability binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNumericPortabilityBinding {
    /// Stable numeric-portability report reference.
    pub report_ref: String,
    /// Stable numeric-portability report identifier.
    pub report_id: String,
    /// Backend families carried by the committed numeric portability matrix.
    pub backend_family_ids: Vec<String>,
    /// Toolchain families carried by the committed numeric portability matrix.
    pub toolchain_family_ids: Vec<String>,
    /// Numeric profile ids carried by the committed numeric portability matrix.
    pub profile_ids: Vec<String>,
}

impl TassadarNumericPortabilityBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar numeric portability binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingNumericPortabilityReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingNumericPortabilityReportId);
        }
        if self.backend_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingNumericPortabilityBackendFamilyIds);
        }
        if self
            .backend_family_ids
            .iter()
            .any(|backend_family_id| backend_family_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidNumericPortabilityBackendFamilyId);
        }
        if self.toolchain_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingNumericPortabilityToolchainFamilyIds);
        }
        if self
            .toolchain_family_ids
            .iter()
            .any(|toolchain_family_id| toolchain_family_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidNumericPortabilityToolchainFamilyId);
        }
        if self.profile_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingNumericPortabilityProfileIds);
        }
        if self
            .profile_ids
            .iter()
            .any(|profile_id| profile_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidNumericPortabilityProfileId);
        }
        Ok(())
    }
}

/// Returns the canonical numeric-portability binding reused by Tassadar
/// environment surfaces.
#[must_use]
pub fn default_tassadar_numeric_portability_binding() -> TassadarNumericPortabilityBinding {
    TassadarNumericPortabilityBinding {
        report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_numeric_portability_report.json",
        ),
        report_id: String::from("tassadar.numeric_portability.report.v1"),
        backend_family_ids: vec![
            String::from("cpu_reference"),
            String::from("cuda_served"),
            String::from("metal_served"),
        ],
        toolchain_family_ids: vec![
            String::from("rustc:wasm32-unknown-unknown"),
            String::from("rustc:wasm32-unknown-unknown+cuda_served"),
            String::from("rustc:wasm32-unknown-unknown+metal_served"),
        ],
        profile_ids: vec![
            String::from("tassadar.numeric_profile.f32_only.v1"),
            String::from("tassadar.numeric_profile.mixed_i32_f32.v1"),
            String::from("tassadar.numeric_profile.bounded_f64_conversion.v1"),
        ],
    }
}

/// Public bounded float-semantics binding reused by Tassadar environment
/// bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFloatSemanticsBinding {
    /// Stable float-semantics report reference.
    pub report_ref: String,
    /// Stable float-semantics report identifier.
    pub report_id: String,
    /// Stable float-semantics family id.
    pub family_id: String,
    /// Stable float-semantics profile id.
    pub profile_id: String,
    /// Stable NaN policy id.
    pub nan_policy_id: String,
    /// Stable canonical NaN bit-pattern string.
    pub canonical_nan_bits_hex: String,
    /// Stable comparison policy id.
    pub comparison_policy_id: String,
    /// Explicit refused regimes outside the bounded scalar-f32 lane.
    pub refused_regime_ids: Vec<String>,
}

impl TassadarFloatSemanticsBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("Tassadar float semantics binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingFloatSemanticsReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingFloatSemanticsReportId);
        }
        if self.family_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingFloatSemanticsFamilyId);
        }
        if self.profile_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingFloatSemanticsProfileId);
        }
        if self.nan_policy_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingFloatSemanticsNanPolicyId);
        }
        if self.canonical_nan_bits_hex.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingFloatSemanticsCanonicalNanBits);
        }
        if self.comparison_policy_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingFloatSemanticsComparisonPolicyId);
        }
        if self.refused_regime_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingFloatSemanticsRefusedRegimes);
        }
        if self
            .refused_regime_ids
            .iter()
            .any(|regime_id| regime_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidFloatSemanticsRefusedRegimeId);
        }
        Ok(())
    }
}

/// Returns the canonical bounded float-semantics binding reused by Tassadar
/// environment surfaces.
#[must_use]
pub fn default_tassadar_float_semantics_binding() -> TassadarFloatSemanticsBinding {
    TassadarFloatSemanticsBinding {
        report_ref: String::from(
            "fixtures/tassadar/reports/tassadar_float_semantics_comparison_matrix_report.json",
        ),
        report_id: String::from("tassadar.float_semantics.comparison_matrix.report.v1"),
        family_id: String::from("tassadar.float_semantics.matrix.v1"),
        profile_id: String::from("tassadar.float_semantics.scalar_f32.v1"),
        nan_policy_id: String::from("canonical_quiet_nan32"),
        canonical_nan_bits_hex: String::from("0x7fc00000"),
        comparison_policy_id: String::from("ordered_wasm_f32"),
        refused_regime_ids: vec![
            String::from("f64_scalar"),
            String::from("nan_payload_preservation"),
            String::from("non_cpu_backend_fast_math"),
        ],
    }
}

/// Public Wasm conformance binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmConformanceBinding {
    /// Stable frozen window report reference.
    pub window_report_ref: String,
    /// Stable frozen window report identifier.
    pub window_report_id: String,
    /// Stable frozen semantic window id.
    pub window_id: String,
    /// Stable official harness id.
    pub official_harness_id: String,
    /// Stable text authority id.
    pub text_authority_id: String,
    /// Stable binary decode authority id.
    pub binary_decode_authority_id: String,
    /// Stable binary encode authority id.
    pub binary_encode_authority_id: String,
    /// Stable validation authority id.
    pub validation_authority_id: String,
    /// Stable Wasm conformance report reference.
    pub report_ref: String,
    /// Stable Wasm conformance report identifier.
    pub report_id: String,
    /// Stable reference-authority identifier.
    pub reference_authority_id: String,
    /// Case families covered by the committed report.
    pub case_family_ids: Vec<String>,
    /// Explicit proposal families outside the frozen window.
    pub unsupported_proposal_family_ids: Vec<String>,
}

impl TassadarWasmConformanceBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("Tassadar Wasm conformance binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.window_report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceWindowReportRef);
        }
        if self.window_report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceWindowReportId);
        }
        if self.window_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceWindowId);
        }
        if self.official_harness_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceOfficialHarnessId);
        }
        if self.text_authority_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceTextAuthorityId);
        }
        if self.binary_decode_authority_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceBinaryDecodeAuthorityId);
        }
        if self.binary_encode_authority_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceBinaryEncodeAuthorityId);
        }
        if self.validation_authority_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceValidationAuthorityId);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceReportRef);
        }
        if self.report_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceReportId);
        }
        if self.reference_authority_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceAuthorityId);
        }
        if self.case_family_ids.is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmConformanceCaseFamilies);
        }
        if self
            .case_family_ids
            .iter()
            .any(|case_family_id| case_family_id.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidWasmConformanceCaseFamilyId);
        }
        if self.unsupported_proposal_family_ids.is_empty() {
            return Err(
                TassadarEnvironmentError::MissingWasmConformanceUnsupportedProposalFamilies,
            );
        }
        if self
            .unsupported_proposal_family_ids
            .iter()
            .any(|proposal_family_id| proposal_family_id.trim().is_empty())
        {
            return Err(
                TassadarEnvironmentError::InvalidWasmConformanceUnsupportedProposalFamilyId,
            );
        }
        Ok(())
    }
}

/// Public module-scale workload-suite binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleScaleWorkloadSuiteBinding {
    /// Stable module-scale suite reference.
    pub suite_ref: String,
    /// Immutable module-scale suite version.
    pub suite_version: String,
    /// Module-scale workload families surfaced by this environment bundle.
    pub supported_families: Vec<TassadarModuleScaleWorkloadFamily>,
    /// Evaluation axes surfaced by the committed report.
    pub evaluation_axes: Vec<String>,
    /// Canonical machine-readable report ref for the suite.
    pub report_ref: String,
}

impl TassadarModuleScaleWorkloadSuiteBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar module-scale workload-suite binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.suite_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingModuleScaleSuiteRef);
        }
        if self.suite_version.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingModuleScaleSuiteVersion);
        }
        if self.supported_families.is_empty() {
            return Err(TassadarEnvironmentError::MissingModuleScaleFamilies);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarEnvironmentError::MissingModuleScaleEvaluationAxes);
        }
        if self
            .evaluation_axes
            .iter()
            .any(|evaluation_axis| evaluation_axis.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidModuleScaleEvaluationAxis);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingModuleScaleReportRef);
        }
        Ok(())
    }
}

/// Public CLRS-to-Wasm bridge binding reused by Tassadar environment bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarClrsWasmBridgeBinding {
    /// Stable bridge reference.
    pub bridge_ref: String,
    /// Immutable bridge version.
    pub bridge_version: String,
    /// CLRS algorithm families surfaced by this environment bundle.
    pub supported_algorithms: Vec<TassadarClrsAlgorithmFamily>,
    /// Trajectory families surfaced by this environment bundle.
    pub trajectory_families: Vec<TassadarClrsTrajectoryFamily>,
    /// Length buckets surfaced by this environment bundle.
    pub length_buckets: Vec<TassadarClrsLengthBucket>,
    /// Canonical machine-readable report ref for the bridge.
    pub report_ref: String,
}

impl TassadarClrsWasmBridgeBinding {
    /// Returns a stable digest over the binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self)
            .expect("Tassadar CLRS-to-Wasm bridge binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.bridge_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingClrsWasmBridgeRef);
        }
        if self.bridge_version.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingClrsWasmBridgeVersion);
        }
        if self.supported_algorithms.is_empty() {
            return Err(TassadarEnvironmentError::MissingClrsWasmBridgeAlgorithms);
        }
        if self.trajectory_families.is_empty() {
            return Err(TassadarEnvironmentError::MissingClrsWasmBridgeTrajectories);
        }
        if self.length_buckets.is_empty() {
            return Err(TassadarEnvironmentError::MissingClrsWasmBridgeLengthBuckets);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingClrsWasmBridgeReportRef);
        }
        Ok(())
    }
}

impl TassadarEnvironmentPackageRefs {
    /// Returns a stable digest over the package refs.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("Tassadar package refs should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the package refs are explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.group_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingGroupRef);
        }
        if self.eval_pin_alias.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingEvalPinAlias);
        }
        if self.benchmark_pin_alias.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkPinAlias);
        }
        if self.eval_member_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingEvalMemberRef);
        }
        if self.benchmark_member_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkMemberRef);
        }
        if self.program_corpus_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingProgramCorpusRef);
        }
        if self.io_contract_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingIoContractRef);
        }
        if self.rubric_binding_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingRubricBindingRef);
        }
        if self.eval_runtime_profile_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingEvalRuntimeProfileRef);
        }
        if self.benchmark_profile_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkProfileRef);
        }
        if self.benchmark_runtime_profile_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkRuntimeProfileRef);
        }
        Ok(())
    }
}

/// Machine-legible binding to the current Tassadar program-artifact set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramBinding {
    /// Stable versioned dataset identity for the corpus.
    pub dataset: DatasetKey,
    /// Stable corpus reference.
    pub program_corpus_ref: String,
    /// Stable digest over the ordered artifact set.
    pub corpus_digest: String,
    /// Stable Wasm profile identifier.
    pub wasm_profile_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI version.
    pub trace_abi_version: u16,
    /// Stable opcode-vocabulary digest.
    pub opcode_vocabulary_digest: String,
    /// Stable program-artifact digests carried by the corpus.
    pub artifact_digests: Vec<String>,
}

impl TassadarProgramBinding {
    /// Returns a stable digest over the program binding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("Tassadar program binding should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the program binding is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.dataset.dataset_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingDatasetRef);
        }
        if self.dataset.version.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingDatasetVersion);
        }
        if self.program_corpus_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingProgramCorpusRef);
        }
        if self.corpus_digest.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingCorpusDigest);
        }
        if self.wasm_profile_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingWasmProfileId);
        }
        if self.trace_abi_id.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingTraceAbiId);
        }
        if self.trace_abi_version == 0 {
            return Err(TassadarEnvironmentError::InvalidTraceAbiVersion);
        }
        if self.opcode_vocabulary_digest.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingOpcodeVocabularyDigest);
        }
        if self.artifact_digests.is_empty() {
            return Err(TassadarEnvironmentError::MissingArtifactDigests);
        }
        if self
            .artifact_digests
            .iter()
            .any(|artifact_digest| artifact_digest.trim().is_empty())
        {
            return Err(TassadarEnvironmentError::InvalidArtifactDigest);
        }
        Ok(())
    }
}

/// Input/output contract bound to one Tassadar environment package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarIoContract {
    /// Stable input family label.
    pub input_family: String,
    /// Stable output family label.
    pub output_family: String,
    /// Stable scalar element type for outputs.
    pub output_element_type: String,
    /// Whether outputs must be deterministic.
    pub deterministic_outputs: bool,
}

impl TassadarIoContract {
    /// Returns the canonical Phase 3 IO contract.
    #[must_use]
    pub fn exact_i32_sequence() -> Self {
        Self {
            input_family: String::from("no_external_input"),
            output_family: String::from("i32_sequence"),
            output_element_type: String::from("i32"),
            deterministic_outputs: true,
        }
    }

    /// Returns a stable digest over the IO contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("Tassadar IO contract should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the IO contract is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.input_family.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingInputFamily);
        }
        if self.output_family.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingOutputFamily);
        }
        if self.output_element_type.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingOutputElementType);
        }
        Ok(())
    }
}

/// Exactness and budget contract for one Tassadar environment bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactnessContract {
    /// Whether final outputs must match exactly.
    pub require_final_output_exactness: bool,
    /// Whether the append-only trace must match exactly.
    pub require_step_exactness: bool,
    /// Whether halt semantics must match exactly.
    pub require_halt_exactness: bool,
    /// Time budget for one case evaluation.
    pub timeout_budget_ms: u64,
    /// Maximum trace length admitted by the package.
    pub trace_budget_steps: u64,
    /// Whether the direct CPU reference baseline is required.
    pub require_cpu_reference_baseline: bool,
    /// Whether the linear reference executor baseline is required.
    pub require_reference_linear_baseline: bool,
    /// Future throughput metric ids declared now but not yet required.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub future_throughput_metric_ids: Vec<String>,
}

impl TassadarExactnessContract {
    /// Returns a stable digest over the exactness contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("Tassadar exactness contract should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the exactness contract is explicit.
    pub fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if !self.require_final_output_exactness {
            return Err(TassadarEnvironmentError::FinalOutputExactnessRequired);
        }
        if !self.require_step_exactness {
            return Err(TassadarEnvironmentError::StepExactnessRequired);
        }
        if !self.require_halt_exactness {
            return Err(TassadarEnvironmentError::HaltExactnessRequired);
        }
        if self.timeout_budget_ms == 0 {
            return Err(TassadarEnvironmentError::InvalidTimeoutBudget);
        }
        if self.trace_budget_steps == 0 {
            return Err(TassadarEnvironmentError::InvalidTraceBudget);
        }
        if !self.require_cpu_reference_baseline {
            return Err(TassadarEnvironmentError::CpuBaselineRequired);
        }
        if !self.require_reference_linear_baseline {
            return Err(TassadarEnvironmentError::ReferenceLinearBaselineRequired);
        }
        Ok(())
    }
}

/// Builder input for a reusable Tassadar eval and benchmark environment bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarEnvironmentSpec {
    /// Immutable bundle version.
    pub version: String,
    /// Shared display label.
    pub display_name: String,
    /// Environment ref for the eval package.
    pub eval_environment_ref: String,
    /// Environment ref for the benchmark package.
    pub benchmark_environment_ref: String,
    /// Eval dataset binding.
    pub eval_dataset: EnvironmentDatasetBinding,
    /// Benchmark dataset binding.
    pub benchmark_dataset: EnvironmentDatasetBinding,
    /// Typed package refs reused across the bundle.
    pub package_refs: TassadarEnvironmentPackageRefs,
    /// Program-corpus binding.
    pub program_binding: TassadarProgramBinding,
    /// Input/output contract.
    pub io_contract: TassadarIoContract,
    /// Exactness and budget contract.
    pub exactness_contract: TassadarExactnessContract,
    /// Public benchmark-package-set binding.
    pub benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding,
    /// Public compile-pipeline matrix binding.
    pub compile_pipeline_matrix_binding: TassadarCompilePipelineMatrixBinding,
    /// Public checkpointed multi-slice execution binding.
    pub execution_checkpoint_binding: TassadarExecutionCheckpointBinding,
    /// Public durable process snapshot, tape, and work-queue binding.
    pub process_object_binding: TassadarProcessObjectBinding,
    /// Public spill-aware continuation and external tape-store binding.
    pub spill_tape_store_binding: TassadarSpillTapeStoreBinding,
    /// Public dynamic-memory pause-and-resume binding.
    pub dynamic_memory_resume_binding: TassadarDynamicMemoryResumeBinding,
    /// Public deterministic simulator-backed effect binding.
    pub simulator_effect_profile_binding: TassadarSimulatorEffectProfileBinding,
    /// Public bounded memory64 continuation binding.
    pub memory64_profile_binding: TassadarMemory64ProfileBinding,
    /// Optional broad internal-compute portability binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broad_internal_compute_portability_binding:
        Option<TassadarBroadInternalComputePortabilityBinding>,
    /// Optional numeric portability binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric_portability_binding: Option<TassadarNumericPortabilityBinding>,
    /// Public bounded float-semantics binding.
    pub float_semantics_binding: TassadarFloatSemanticsBinding,
    /// Public Wasm conformance binding.
    pub wasm_conformance_binding: TassadarWasmConformanceBinding,
    /// Optional broadened architecture-bakeoff binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture_bakeoff_binding: Option<TassadarArchitectureBakeoffBinding>,
    /// Optional public module-scale workload-suite binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_scale_workload_suite_binding: Option<TassadarModuleScaleWorkloadSuiteBinding>,
    /// Optional public CLRS-to-Wasm bridge binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clrs_wasm_bridge_binding: Option<TassadarClrsWasmBridgeBinding>,
    /// Policy refs for the eval package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eval_policy_references: Vec<EnvironmentPolicyReference>,
    /// Policy refs for the benchmark package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub benchmark_policy_references: Vec<EnvironmentPolicyReference>,
    /// Current workload targets implemented by the corpus.
    pub current_workload_targets: Vec<TassadarWorkloadTarget>,
    /// Declared future workload targets that should reuse the same package family.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub planned_workload_targets: Vec<TassadarWorkloadTarget>,
}

impl TassadarEnvironmentSpec {
    /// Builds the reusable Tassadar environment bundle.
    pub fn build_bundle(&self) -> Result<TassadarEnvironmentBundle, TassadarEnvironmentError> {
        self.validate()?;
        let eval_package = self.eval_package()?;
        let benchmark_package = self.benchmark_package()?;
        let group = self.group_definition();

        let mut registry = EnvironmentRegistry::default();
        registry
            .install_package(crate::EnvironmentInstallRequest {
                package: eval_package.clone(),
                source: EnvironmentPackageInstallSource::BuiltIn {
                    owner: String::from("tassadar_environment_bundle"),
                },
                dependencies: Vec::new(),
            })
            .map_err(TassadarEnvironmentError::Registry)?;
        registry
            .install_package(crate::EnvironmentInstallRequest {
                package: benchmark_package.clone(),
                source: EnvironmentPackageInstallSource::BuiltIn {
                    owner: String::from("tassadar_environment_bundle"),
                },
                dependencies: vec![eval_package.key.clone()],
            })
            .map_err(TassadarEnvironmentError::Registry)?;
        registry
            .pin_package(
                self.package_refs.eval_pin_alias.clone(),
                eval_package.key.clone(),
                vec![
                    EnvironmentWorkloadClass::OnlineEval,
                    EnvironmentWorkloadClass::OfflineEval,
                ],
            )
            .map_err(TassadarEnvironmentError::Registry)?;
        registry
            .pin_package(
                self.package_refs.benchmark_pin_alias.clone(),
                benchmark_package.key.clone(),
                vec![EnvironmentWorkloadClass::ValidatorBenchmark],
            )
            .map_err(TassadarEnvironmentError::Registry)?;
        registry
            .define_group(group.clone())
            .map_err(TassadarEnvironmentError::Registry)?;

        let eval_resolution = registry
            .resolve_group(
                self.package_refs.group_ref.as_str(),
                EnvironmentUsageSurface::Eval,
            )
            .map_err(TassadarEnvironmentError::Registry)?;
        let benchmark_resolution = registry
            .resolve_group(
                self.package_refs.group_ref.as_str(),
                EnvironmentUsageSurface::Benchmark,
            )
            .map_err(TassadarEnvironmentError::Registry)?;

        Ok(TassadarEnvironmentBundle {
            eval_package,
            benchmark_package,
            group,
            eval_resolution,
            benchmark_resolution,
            package_refs: self.package_refs.clone(),
            program_binding: self.program_binding.clone(),
            io_contract: self.io_contract.clone(),
            exactness_contract: self.exactness_contract.clone(),
            benchmark_package_set_binding: self.benchmark_package_set_binding.clone(),
            compile_pipeline_matrix_binding: self.compile_pipeline_matrix_binding.clone(),
            execution_checkpoint_binding: self.execution_checkpoint_binding.clone(),
            process_object_binding: self.process_object_binding.clone(),
            spill_tape_store_binding: self.spill_tape_store_binding.clone(),
            dynamic_memory_resume_binding: self.dynamic_memory_resume_binding.clone(),
            simulator_effect_profile_binding: self.simulator_effect_profile_binding.clone(),
            memory64_profile_binding: self.memory64_profile_binding.clone(),
            broad_internal_compute_portability_binding: self
                .broad_internal_compute_portability_binding
                .clone(),
            numeric_portability_binding: self.numeric_portability_binding.clone(),
            float_semantics_binding: self.float_semantics_binding.clone(),
            rust_article_profile_completeness:
                tassadar_rust_article_profile_completeness_publication(),
            generalized_abi_family: tassadar_generalized_abi_publication(),
            internal_compute_profile_ladder: tassadar_internal_compute_profile_ladder_publication(),
            internal_compute_profile_claim_check: check_tassadar_internal_compute_profile_claim(
                &tassadar_internal_compute_profile_ladder_publication(),
                tassadar_current_served_internal_compute_profile_claim(),
            ),
            wasm_conformance_binding: self.wasm_conformance_binding.clone(),
            architecture_bakeoff_binding: self.architecture_bakeoff_binding.clone(),
            module_scale_workload_suite_binding: self.module_scale_workload_suite_binding.clone(),
            clrs_wasm_bridge_binding: self.clrs_wasm_bridge_binding.clone(),
            current_workload_targets: self.current_workload_targets.clone(),
            planned_workload_targets: self.planned_workload_targets.clone(),
        })
    }

    fn validate(&self) -> Result<(), TassadarEnvironmentError> {
        if self.version.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingVersion);
        }
        if self.display_name.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingDisplayName);
        }
        if self.eval_environment_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingEvalEnvironmentRef);
        }
        if self.benchmark_environment_ref.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkEnvironmentRef);
        }
        if self.eval_environment_ref == self.benchmark_environment_ref {
            return Err(TassadarEnvironmentError::DuplicateEnvironmentRef);
        }
        if self.eval_dataset.mount_path.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingEvalDataset);
        }
        if self.benchmark_dataset.mount_path.trim().is_empty() {
            return Err(TassadarEnvironmentError::MissingBenchmarkDataset);
        }
        self.package_refs.validate()?;
        self.program_binding.validate()?;
        self.io_contract.validate()?;
        self.exactness_contract.validate()?;
        self.benchmark_package_set_binding.validate()?;
        self.compile_pipeline_matrix_binding.validate()?;
        self.execution_checkpoint_binding.validate()?;
        self.process_object_binding.validate()?;
        self.spill_tape_store_binding.validate()?;
        self.dynamic_memory_resume_binding.validate()?;
        self.simulator_effect_profile_binding.validate()?;
        self.memory64_profile_binding.validate()?;
        if let Some(binding) = &self.broad_internal_compute_portability_binding {
            binding.validate()?;
        }
        if let Some(binding) = &self.numeric_portability_binding {
            binding.validate()?;
        }
        self.float_semantics_binding.validate()?;
        self.wasm_conformance_binding.validate()?;
        if let Some(binding) = &self.architecture_bakeoff_binding {
            binding.validate()?;
        }
        if let Some(binding) = &self.module_scale_workload_suite_binding {
            binding.validate()?;
        }
        if let Some(binding) = &self.clrs_wasm_bridge_binding {
            binding.validate()?;
        }
        if self.current_workload_targets.is_empty() {
            return Err(TassadarEnvironmentError::MissingCurrentWorkloadTargets);
        }
        if !self
            .eval_policy_references
            .iter()
            .any(|policy| policy.kind == EnvironmentPolicyKind::Verification)
        {
            return Err(TassadarEnvironmentError::MissingEvalVerificationPolicyRef);
        }
        if !self
            .benchmark_policy_references
            .iter()
            .any(|policy| policy.kind == EnvironmentPolicyKind::Benchmark)
        {
            return Err(TassadarEnvironmentError::MissingBenchmarkPolicyRef);
        }
        if !self
            .benchmark_policy_references
            .iter()
            .any(|policy| policy.kind == EnvironmentPolicyKind::Verification)
        {
            return Err(TassadarEnvironmentError::MissingBenchmarkVerificationPolicyRef);
        }
        Ok(())
    }

    fn eval_package(&self) -> Result<EnvironmentPackageContract, TassadarEnvironmentError> {
        let package = EnvironmentPackageContract::new(
            EnvironmentPackageKey::new(self.eval_environment_ref.clone(), self.version.clone()),
            EnvironmentPackageFamily::Evaluation,
            format!("{} Eval", self.display_name),
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::Evaluator,
                entrypoint: String::from("tassadar_eval::run"),
                args: vec![self.package_refs.eval_runtime_profile_ref.clone()],
                sandbox_profile_ref: None,
                max_turns: 1,
                state_mode: EnvironmentStateMode::Stateless,
                time_budget_ms: Some(self.exactness_contract.timeout_budget_ms),
            },
        )
        .with_supported_workloads(vec![
            EnvironmentWorkloadClass::OnlineEval,
            EnvironmentWorkloadClass::OfflineEval,
        ])
        .with_datasets(vec![self.eval_dataset.clone()])
        .with_rubric_hooks(self.rubric_hooks())
        .with_expected_artifacts(self.expected_artifacts())
        .with_policy_references(self.eval_policy_references.clone())
        .with_metadata(self.shared_metadata(TASSADAR_EVAL_METADATA_SURFACE));
        package
            .validate()
            .map_err(TassadarEnvironmentError::Contract)?;
        Ok(package)
    }

    fn benchmark_package(&self) -> Result<EnvironmentPackageContract, TassadarEnvironmentError> {
        let package = EnvironmentPackageContract::new(
            EnvironmentPackageKey::new(
                self.benchmark_environment_ref.clone(),
                self.version.clone(),
            ),
            EnvironmentPackageFamily::Evaluation,
            format!("{} Benchmark", self.display_name),
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::Evaluator,
                entrypoint: String::from("tassadar_benchmark::run"),
                args: vec![self.package_refs.benchmark_runtime_profile_ref.clone()],
                sandbox_profile_ref: None,
                max_turns: 1,
                state_mode: EnvironmentStateMode::TurnScoped,
                time_budget_ms: Some(self.exactness_contract.timeout_budget_ms),
            },
        )
        .with_supported_workloads(vec![
            EnvironmentWorkloadClass::OfflineEval,
            EnvironmentWorkloadClass::ValidatorBenchmark,
        ])
        .with_datasets(vec![self.benchmark_dataset.clone()])
        .with_rubric_hooks(self.rubric_hooks())
        .with_expected_artifacts(self.expected_artifacts())
        .with_policy_references(self.benchmark_policy_references.clone())
        .with_benchmark_profiles(vec![EnvironmentBenchmarkProfile {
            benchmark_profile_ref: self.package_refs.benchmark_profile_ref.clone(),
            runtime_profile_ref: self.package_refs.benchmark_runtime_profile_ref.clone(),
            verification_posture: EnvironmentVerificationPosture::ValidatorRequired,
            expected_execution_strategy: Some(String::from("tassadar_reference_fixture")),
        }])
        .with_metadata(self.shared_metadata(TASSADAR_BENCHMARK_METADATA_SURFACE));
        package
            .validate()
            .map_err(TassadarEnvironmentError::Contract)?;
        Ok(package)
    }

    fn rubric_hooks(&self) -> Vec<EnvironmentRubricHook> {
        vec![
            EnvironmentRubricHook {
                rubric_ref: format!(
                    "{}/final_output_exactness",
                    self.package_refs.rubric_binding_ref
                ),
                hook_name: String::from("score_final_output_exactness"),
                score_kind: crate::EnvironmentRubricScoreKind::Binary,
                pass_threshold: Some(10_000),
            },
            EnvironmentRubricHook {
                rubric_ref: format!("{}/step_exactness", self.package_refs.rubric_binding_ref),
                hook_name: String::from("score_step_exactness"),
                score_kind: crate::EnvironmentRubricScoreKind::Binary,
                pass_threshold: Some(10_000),
            },
            EnvironmentRubricHook {
                rubric_ref: format!("{}/halt_exactness", self.package_refs.rubric_binding_ref),
                hook_name: String::from("score_halt_exactness"),
                score_kind: crate::EnvironmentRubricScoreKind::Binary,
                pass_threshold: Some(10_000),
            },
        ]
    }

    fn expected_artifacts(&self) -> Vec<crate::EnvironmentArtifactExpectation> {
        vec![
            crate::EnvironmentArtifactExpectation {
                artifact_kind: String::from("tassadar_program_artifact.json"),
                required: true,
                verification_policy_ref: Some(String::from("policy://tassadar/program_artifact")),
            },
            crate::EnvironmentArtifactExpectation {
                artifact_kind: String::from("tassadar_trace.json"),
                required: true,
                verification_policy_ref: Some(String::from("policy://tassadar/trace")),
            },
            crate::EnvironmentArtifactExpectation {
                artifact_kind: String::from("tassadar_eval_report.json"),
                required: true,
                verification_policy_ref: Some(String::from("policy://tassadar/eval_report")),
            },
        ]
    }

    fn shared_metadata(&self, surface: &str) -> BTreeMap<String, Value> {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            String::from(TASSADAR_METADATA_ABI_VERSION_KEY),
            Value::String(String::from(TASSADAR_ENVIRONMENT_ABI_VERSION)),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_SURFACE_KEY),
            Value::String(String::from(surface)),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_PACKAGE_REFS_KEY),
            serde_json::to_value(&self.package_refs).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_PROGRAM_BINDING_KEY),
            serde_json::to_value(&self.program_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_IO_CONTRACT_KEY),
            serde_json::to_value(&self.io_contract).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_EXACTNESS_CONTRACT_KEY),
            serde_json::to_value(&self.exactness_contract).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_BENCHMARK_PACKAGE_SET_KEY),
            serde_json::to_value(&self.benchmark_package_set_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_COMPILE_PIPELINE_MATRIX_KEY),
            serde_json::to_value(&self.compile_pipeline_matrix_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_RUST_ARTICLE_PROFILE_KEY),
            serde_json::to_value(tassadar_rust_article_profile_completeness_publication())
                .unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_GENERALIZED_ABI_FAMILY_KEY),
            serde_json::to_value(tassadar_generalized_abi_publication()).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_EXECUTION_CHECKPOINT_KEY),
            serde_json::to_value(&self.execution_checkpoint_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_PROCESS_OBJECT_KEY),
            serde_json::to_value(&self.process_object_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_SPILL_TAPE_STORE_KEY),
            serde_json::to_value(&self.spill_tape_store_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_DYNAMIC_MEMORY_RESUME_KEY),
            serde_json::to_value(&self.dynamic_memory_resume_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_SIMULATOR_EFFECT_PROFILE_KEY),
            serde_json::to_value(&self.simulator_effect_profile_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_MEMORY64_PROFILE_KEY),
            serde_json::to_value(&self.memory64_profile_binding).unwrap_or(Value::Null),
        );
        let internal_compute_profile_ladder =
            tassadar_internal_compute_profile_ladder_publication();
        metadata.insert(
            String::from(TASSADAR_METADATA_INTERNAL_COMPUTE_PROFILE_LADDER_KEY),
            serde_json::to_value(&internal_compute_profile_ladder).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_INTERNAL_COMPUTE_PROFILE_CLAIM_KEY),
            serde_json::to_value(check_tassadar_internal_compute_profile_claim(
                &internal_compute_profile_ladder,
                tassadar_current_served_internal_compute_profile_claim(),
            ))
            .unwrap_or(Value::Null),
        );
        if let Some(binding) = &self.broad_internal_compute_portability_binding {
            metadata.insert(
                String::from(TASSADAR_METADATA_BROAD_INTERNAL_COMPUTE_PORTABILITY_KEY),
                serde_json::to_value(binding).unwrap_or(Value::Null),
            );
        }
        if let Some(binding) = &self.numeric_portability_binding {
            metadata.insert(
                String::from(TASSADAR_METADATA_NUMERIC_PORTABILITY_KEY),
                serde_json::to_value(binding).unwrap_or(Value::Null),
            );
        }
        metadata.insert(
            String::from(TASSADAR_METADATA_FLOAT_SEMANTICS_KEY),
            serde_json::to_value(&self.float_semantics_binding).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_WASM_CONFORMANCE_KEY),
            serde_json::to_value(&self.wasm_conformance_binding).unwrap_or(Value::Null),
        );
        if let Some(binding) = &self.architecture_bakeoff_binding {
            metadata.insert(
                String::from(TASSADAR_METADATA_ARCHITECTURE_BAKEOFF_KEY),
                serde_json::to_value(binding).unwrap_or(Value::Null),
            );
        }
        if let Some(binding) = &self.module_scale_workload_suite_binding {
            metadata.insert(
                String::from(TASSADAR_METADATA_MODULE_SCALE_WORKLOAD_SUITE_KEY),
                serde_json::to_value(binding).unwrap_or(Value::Null),
            );
        }
        if let Some(binding) = &self.clrs_wasm_bridge_binding {
            metadata.insert(
                String::from(TASSADAR_METADATA_CLRS_WASM_BRIDGE_KEY),
                serde_json::to_value(binding).unwrap_or(Value::Null),
            );
        }
        metadata.insert(
            String::from(TASSADAR_METADATA_CURRENT_TARGETS_KEY),
            serde_json::to_value(&self.current_workload_targets).unwrap_or(Value::Null),
        );
        metadata.insert(
            String::from(TASSADAR_METADATA_PLANNED_TARGETS_KEY),
            serde_json::to_value(&self.planned_workload_targets).unwrap_or(Value::Null),
        );
        metadata
    }

    fn group_definition(&self) -> EnvironmentCompositionGroup {
        EnvironmentCompositionGroup {
            group_ref: self.package_refs.group_ref.clone(),
            display_name: self.display_name.clone(),
            members: vec![
                EnvironmentCompositionMember {
                    member_ref: self.package_refs.eval_member_ref.clone(),
                    pin_alias: self.package_refs.eval_pin_alias.clone(),
                    surfaces: vec![EnvironmentUsageSurface::Eval],
                    required_workloads: vec![
                        EnvironmentWorkloadClass::OnlineEval,
                        EnvironmentWorkloadClass::OfflineEval,
                    ],
                    required_benchmark_profiles: Vec::new(),
                },
                EnvironmentCompositionMember {
                    member_ref: self.package_refs.benchmark_member_ref.clone(),
                    pin_alias: self.package_refs.benchmark_pin_alias.clone(),
                    surfaces: vec![EnvironmentUsageSurface::Benchmark],
                    required_workloads: vec![EnvironmentWorkloadClass::ValidatorBenchmark],
                    required_benchmark_profiles: vec![
                        self.package_refs.benchmark_profile_ref.clone(),
                    ],
                },
            ],
        }
    }
}

/// Resolved reusable Tassadar environment bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarEnvironmentBundle {
    /// Eval package.
    pub eval_package: EnvironmentPackageContract,
    /// Benchmark package.
    pub benchmark_package: EnvironmentPackageContract,
    /// Mixed-surface environment group.
    pub group: EnvironmentCompositionGroup,
    /// Eval-surface resolution.
    pub eval_resolution: EnvironmentGroupResolution,
    /// Benchmark-surface resolution.
    pub benchmark_resolution: EnvironmentGroupResolution,
    /// Shared package refs.
    pub package_refs: TassadarEnvironmentPackageRefs,
    /// Program binding.
    pub program_binding: TassadarProgramBinding,
    /// IO contract.
    pub io_contract: TassadarIoContract,
    /// Exactness contract.
    pub exactness_contract: TassadarExactnessContract,
    /// Benchmark-package-set binding.
    pub benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding,
    /// Compile-pipeline matrix binding.
    pub compile_pipeline_matrix_binding: TassadarCompilePipelineMatrixBinding,
    /// Checkpointed multi-slice execution binding.
    pub execution_checkpoint_binding: TassadarExecutionCheckpointBinding,
    /// Durable process snapshot, tape, and work-queue binding.
    pub process_object_binding: TassadarProcessObjectBinding,
    /// Spill-aware continuation and external tape-store binding.
    pub spill_tape_store_binding: TassadarSpillTapeStoreBinding,
    /// Dynamic-memory pause-and-resume binding.
    pub dynamic_memory_resume_binding: TassadarDynamicMemoryResumeBinding,
    /// Deterministic simulator-backed effect binding.
    pub simulator_effect_profile_binding: TassadarSimulatorEffectProfileBinding,
    /// Bounded memory64 continuation binding.
    pub memory64_profile_binding: TassadarMemory64ProfileBinding,
    /// Optional broad internal-compute portability binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broad_internal_compute_portability_binding:
        Option<TassadarBroadInternalComputePortabilityBinding>,
    /// Optional numeric portability binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric_portability_binding: Option<TassadarNumericPortabilityBinding>,
    /// Bounded float-semantics binding.
    pub float_semantics_binding: TassadarFloatSemanticsBinding,
    /// Rust-to-Wasm article profile completeness publication.
    pub rust_article_profile_completeness: TassadarRustArticleProfileCompletenessPublication,
    /// Generalized ABI family publication.
    pub generalized_abi_family: TassadarGeneralizedAbiPublication,
    /// Named post-article internal-compute profile ladder publication.
    pub internal_compute_profile_ladder: TassadarInternalComputeProfileLadderPublication,
    /// Current environment-bound internal-compute claim-check result.
    pub internal_compute_profile_claim_check: TassadarInternalComputeProfileClaimCheckResult,
    /// Wasm conformance binding.
    pub wasm_conformance_binding: TassadarWasmConformanceBinding,
    /// Optional broadened architecture-bakeoff binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture_bakeoff_binding: Option<TassadarArchitectureBakeoffBinding>,
    /// Optional module-scale workload-suite binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_scale_workload_suite_binding: Option<TassadarModuleScaleWorkloadSuiteBinding>,
    /// Optional CLRS-to-Wasm bridge binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clrs_wasm_bridge_binding: Option<TassadarClrsWasmBridgeBinding>,
    /// Current workload targets implemented now.
    pub current_workload_targets: Vec<TassadarWorkloadTarget>,
    /// Planned workload targets still to widen later.
    pub planned_workload_targets: Vec<TassadarWorkloadTarget>,
}

/// Tassadar environment spec/build failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarEnvironmentError {
    /// Missing bundle version.
    #[error("Tassadar environment spec is missing `version`")]
    MissingVersion,
    /// Missing display name.
    #[error("Tassadar environment spec is missing `display_name`")]
    MissingDisplayName,
    /// Missing eval environment ref.
    #[error("Tassadar environment spec is missing `eval_environment_ref`")]
    MissingEvalEnvironmentRef,
    /// Missing benchmark environment ref.
    #[error("Tassadar environment spec is missing `benchmark_environment_ref`")]
    MissingBenchmarkEnvironmentRef,
    /// Eval and benchmark refs must not match.
    #[error("Tassadar environment spec must use distinct eval and benchmark refs")]
    DuplicateEnvironmentRef,
    /// Missing eval dataset.
    #[error("Tassadar environment spec is missing the eval dataset binding")]
    MissingEvalDataset,
    /// Missing benchmark dataset.
    #[error("Tassadar environment spec is missing the benchmark dataset binding")]
    MissingBenchmarkDataset,
    /// Missing benchmark package-set ref.
    #[error("Tassadar environment spec is missing `benchmark_package_set_binding.package_set_ref`")]
    MissingBenchmarkPackageSetRef,
    /// Missing benchmark package-set version.
    #[error(
        "Tassadar environment spec is missing `benchmark_package_set_binding.package_set_version`"
    )]
    MissingBenchmarkPackageSetVersion,
    /// Missing benchmark families.
    #[error("Tassadar environment spec must declare at least one benchmark package-set family")]
    MissingBenchmarkFamilies,
    /// Missing benchmark axis coverage.
    #[error("Tassadar environment spec must declare benchmark axis coverage")]
    MissingBenchmarkAxisCoverage,
    /// Missing benchmark summary report ref.
    #[error(
        "Tassadar environment spec is missing `benchmark_package_set_binding.summary_report_ref`"
    )]
    MissingBenchmarkSummaryReportRef,
    /// Missing compile-pipeline matrix report ref.
    #[error("Tassadar environment spec is missing `compile_pipeline_matrix_binding.report_ref`")]
    MissingCompilePipelineMatrixReportRef,
    /// Missing compile-pipeline matrix report id.
    #[error("Tassadar environment spec is missing `compile_pipeline_matrix_binding.report_id`")]
    MissingCompilePipelineMatrixReportId,
    /// Missing compile-pipeline matrix source families.
    #[error("Tassadar environment spec must declare compile-pipeline source families")]
    MissingCompilePipelineSourceFamilies,
    /// Invalid compile-pipeline matrix source family id.
    #[error("Tassadar environment spec includes an empty compile-pipeline source family id")]
    InvalidCompilePipelineSourceFamilyId,
    /// Missing execution-checkpoint report ref.
    #[error("Tassadar environment spec is missing `execution_checkpoint_binding.report_ref`")]
    MissingExecutionCheckpointReportRef,
    /// Missing execution-checkpoint report id.
    #[error("Tassadar environment spec is missing `execution_checkpoint_binding.report_id`")]
    MissingExecutionCheckpointReportId,
    /// Missing execution-checkpoint run-bundle ref.
    #[error("Tassadar environment spec is missing `execution_checkpoint_binding.run_bundle_ref`")]
    MissingExecutionCheckpointRunBundleRef,
    /// Missing execution-checkpoint family id.
    #[error(
        "Tassadar environment spec is missing `execution_checkpoint_binding.checkpoint_family_id`"
    )]
    MissingExecutionCheckpointFamilyId,
    /// Missing execution-checkpoint workload coverage.
    #[error(
        "Tassadar environment spec is missing `execution_checkpoint_binding.workload_family_ids`"
    )]
    MissingExecutionCheckpointWorkloadFamilies,
    /// Invalid execution-checkpoint workload family id.
    #[error("Tassadar environment spec includes an empty execution-checkpoint workload family id")]
    InvalidExecutionCheckpointWorkloadFamilyId,
    /// Missing dynamic-memory resume report ref.
    #[error("Tassadar environment spec is missing `process_object_binding.report_ref`")]
    MissingProcessObjectReportRef,
    /// Missing durable process report id.
    #[error("Tassadar environment spec is missing `process_object_binding.report_id`")]
    MissingProcessObjectReportId,
    /// Missing durable process run-bundle ref.
    #[error("Tassadar environment spec is missing `process_object_binding.run_bundle_ref`")]
    MissingProcessObjectRunBundleRef,
    /// Missing durable process snapshot family id.
    #[error("Tassadar environment spec is missing `process_object_binding.snapshot_family_id`")]
    MissingProcessSnapshotFamilyId,
    /// Missing durable process tape family id.
    #[error("Tassadar environment spec is missing `process_object_binding.tape_family_id`")]
    MissingProcessTapeFamilyId,
    /// Missing durable process work-queue family id.
    #[error("Tassadar environment spec is missing `process_object_binding.work_queue_family_id`")]
    MissingProcessWorkQueueFamilyId,
    /// Missing durable process ids.
    #[error("Tassadar environment spec is missing `process_object_binding.process_ids`")]
    MissingProcessObjectProcessIds,
    /// Invalid durable process id.
    #[error("Tassadar environment spec includes an empty process-object process id")]
    InvalidProcessObjectProcessId,
    /// Missing spill/tape report ref.
    #[error("Tassadar environment spec is missing `spill_tape_store_binding.report_ref`")]
    MissingSpillTapeStoreReportRef,
    /// Missing spill/tape report id.
    #[error("Tassadar environment spec is missing `spill_tape_store_binding.report_id`")]
    MissingSpillTapeStoreReportId,
    /// Missing spill/tape run-bundle ref.
    #[error("Tassadar environment spec is missing `spill_tape_store_binding.run_bundle_ref`")]
    MissingSpillTapeStoreRunBundleRef,
    /// Missing spill/tape profile id.
    #[error("Tassadar environment spec is missing `spill_tape_store_binding.profile_id`")]
    MissingSpillTapeStoreProfileId,
    /// Missing spill-segment family id.
    #[error(
        "Tassadar environment spec is missing `spill_tape_store_binding.spill_segment_family_id`"
    )]
    MissingSpillSegmentFamilyId,
    /// Missing external tape-store family id.
    #[error(
        "Tassadar environment spec is missing `spill_tape_store_binding.external_tape_store_family_id`"
    )]
    MissingExternalTapeStoreFamilyId,
    /// Missing spill/tape portability envelopes.
    #[error(
        "Tassadar environment spec is missing `spill_tape_store_binding.portability_envelope_ids`"
    )]
    MissingSpillTapeStorePortabilityEnvelopes,
    /// Invalid spill/tape portability envelope id.
    #[error(
        "Tassadar environment spec includes an empty spill/tape portability envelope id"
    )]
    InvalidSpillTapeStorePortabilityEnvelopeId,
    /// Missing spill/tape case ids.
    #[error("Tassadar environment spec is missing `spill_tape_store_binding.case_ids`")]
    MissingSpillTapeStoreCaseIds,
    /// Invalid spill/tape case id.
    #[error("Tassadar environment spec includes an empty spill/tape case id")]
    InvalidSpillTapeStoreCaseId,
    /// Missing dynamic-memory resume report ref.
    #[error("Tassadar environment spec is missing `dynamic_memory_resume_binding.report_ref`")]
    MissingDynamicMemoryResumeReportRef,
    /// Missing dynamic-memory resume report id.
    #[error("Tassadar environment spec is missing `dynamic_memory_resume_binding.report_id`")]
    MissingDynamicMemoryResumeReportId,
    /// Missing dynamic-memory resume run-bundle ref.
    #[error("Tassadar environment spec is missing `dynamic_memory_resume_binding.run_bundle_ref`")]
    MissingDynamicMemoryResumeRunBundleRef,
    /// Missing dynamic-memory resume family id.
    #[error(
        "Tassadar environment spec is missing `dynamic_memory_resume_binding.checkpoint_family_id`"
    )]
    MissingDynamicMemoryResumeFamilyId,
    /// Missing dynamic-memory resume case ids.
    #[error("Tassadar environment spec is missing `dynamic_memory_resume_binding.case_ids`")]
    MissingDynamicMemoryResumeCaseIds,
    /// Invalid dynamic-memory resume case id.
    #[error("Tassadar environment spec includes an empty dynamic-memory resume case id")]
    InvalidDynamicMemoryResumeCaseId,
    /// Missing simulator-effect profile report ref.
    #[error("Tassadar environment spec is missing `simulator_effect_profile_binding.report_ref`")]
    MissingSimulatorEffectProfileReportRef,
    /// Missing simulator-effect profile report id.
    #[error("Tassadar environment spec is missing `simulator_effect_profile_binding.report_id`")]
    MissingSimulatorEffectProfileReportId,
    /// Missing simulator-effect profile run-bundle ref.
    #[error(
        "Tassadar environment spec is missing `simulator_effect_profile_binding.run_bundle_ref`"
    )]
    MissingSimulatorEffectProfileRunBundleRef,
    /// Missing simulator-effect profile id.
    #[error("Tassadar environment spec is missing `simulator_effect_profile_binding.profile_id`")]
    MissingSimulatorEffectProfileId,
    /// Missing simulator profile ids.
    #[error(
        "Tassadar environment spec is missing `simulator_effect_profile_binding.simulator_profile_ids`"
    )]
    MissingSimulatorEffectProfileIds,
    /// Invalid simulator profile id.
    #[error("Tassadar environment spec includes an empty simulator-effect profile id")]
    InvalidSimulatorEffectProfileId,
    /// Missing simulator-effect portability envelopes.
    #[error(
        "Tassadar environment spec is missing `simulator_effect_profile_binding.portability_envelope_ids`"
    )]
    MissingSimulatorEffectPortabilityEnvelopes,
    /// Invalid simulator-effect portability envelope id.
    #[error(
        "Tassadar environment spec includes an empty simulator-effect portability envelope id"
    )]
    InvalidSimulatorEffectPortabilityEnvelopeId,
    /// Missing simulator-effect refused effect ids.
    #[error(
        "Tassadar environment spec is missing `simulator_effect_profile_binding.refused_effect_ids`"
    )]
    MissingSimulatorEffectRefusedEffectIds,
    /// Invalid simulator-effect refused effect id.
    #[error("Tassadar environment spec includes an empty simulator-effect refused effect id")]
    InvalidSimulatorEffectRefusedEffectId,
    /// Missing simulator-effect case ids.
    #[error(
        "Tassadar environment spec is missing `simulator_effect_profile_binding.case_ids`"
    )]
    MissingSimulatorEffectCaseIds,
    /// Invalid simulator-effect case id.
    #[error("Tassadar environment spec includes an empty simulator-effect case id")]
    InvalidSimulatorEffectCaseId,
    /// Missing memory64 profile report ref.
    #[error("Tassadar environment spec is missing `memory64_profile_binding.report_ref`")]
    MissingMemory64ProfileReportRef,
    /// Missing memory64 profile report id.
    #[error("Tassadar environment spec is missing `memory64_profile_binding.report_id`")]
    MissingMemory64ProfileReportId,
    /// Missing memory64 profile run-bundle ref.
    #[error("Tassadar environment spec is missing `memory64_profile_binding.run_bundle_ref`")]
    MissingMemory64ProfileRunBundleRef,
    /// Missing memory64 profile family id.
    #[error("Tassadar environment spec is missing `memory64_profile_binding.checkpoint_family_id`")]
    MissingMemory64ProfileFamilyId,
    /// Missing memory64 profile id.
    #[error("Tassadar environment spec is missing `memory64_profile_binding.profile_id`")]
    MissingMemory64ProfileId,
    /// Missing memory64 portability envelope id.
    #[error(
        "Tassadar environment spec is missing `memory64_profile_binding.portability_envelope_id`"
    )]
    MissingMemory64PortabilityEnvelopeId,
    /// Invalid memory64 address width.
    #[error(
        "Tassadar environment spec has invalid `memory64_profile_binding.max_virtual_address_bits`"
    )]
    InvalidMemory64ProfileAddressBits,
    /// Missing memory64 case ids.
    #[error("Tassadar environment spec is missing `memory64_profile_binding.case_ids`")]
    MissingMemory64ProfileCaseIds,
    /// Invalid memory64 case id.
    #[error("Tassadar environment spec includes an empty memory64 profile case id")]
    InvalidMemory64ProfileCaseId,
    /// Missing broad portability report ref.
    #[error(
        "Tassadar environment spec is missing `broad_internal_compute_portability_binding.report_ref`"
    )]
    MissingBroadInternalComputePortabilityReportRef,
    /// Missing broad portability report id.
    #[error(
        "Tassadar environment spec is missing `broad_internal_compute_portability_binding.report_id`"
    )]
    MissingBroadInternalComputePortabilityReportId,
    /// Missing broad acceptance-gate ref.
    #[error(
        "Tassadar environment spec is missing `broad_internal_compute_portability_binding.acceptance_gate_ref`"
    )]
    MissingBroadInternalComputeAcceptanceGateRef,
    /// Missing broad acceptance-gate id.
    #[error(
        "Tassadar environment spec is missing `broad_internal_compute_portability_binding.acceptance_gate_id`"
    )]
    MissingBroadInternalComputeAcceptanceGateId,
    /// Missing broad internal-compute backend families.
    #[error(
        "Tassadar environment spec is missing `broad_internal_compute_portability_binding.backend_family_ids`"
    )]
    MissingBroadInternalComputeBackendFamilyIds,
    /// Invalid broad internal-compute backend family id.
    #[error("Tassadar environment spec includes an empty broad internal-compute backend family id")]
    InvalidBroadInternalComputeBackendFamilyId,
    /// Missing broad internal-compute toolchain families.
    #[error(
        "Tassadar environment spec is missing `broad_internal_compute_portability_binding.toolchain_family_ids`"
    )]
    MissingBroadInternalComputeToolchainFamilyIds,
    /// Invalid broad internal-compute toolchain family id.
    #[error(
        "Tassadar environment spec includes an empty broad internal-compute toolchain family id"
    )]
    InvalidBroadInternalComputeToolchainFamilyId,
    /// Missing broad internal-compute profile ids.
    #[error(
        "Tassadar environment spec is missing `broad_internal_compute_portability_binding.profile_ids`"
    )]
    MissingBroadInternalComputeProfileIds,
    /// Invalid broad internal-compute profile id.
    #[error("Tassadar environment spec includes an empty broad internal-compute profile id")]
    InvalidBroadInternalComputeProfileId,
    /// Missing numeric portability report ref.
    #[error("Tassadar environment spec is missing `numeric_portability_binding.report_ref`")]
    MissingNumericPortabilityReportRef,
    /// Missing numeric portability report id.
    #[error("Tassadar environment spec is missing `numeric_portability_binding.report_id`")]
    MissingNumericPortabilityReportId,
    /// Missing numeric portability backend families.
    #[error(
        "Tassadar environment spec is missing `numeric_portability_binding.backend_family_ids`"
    )]
    MissingNumericPortabilityBackendFamilyIds,
    /// Invalid numeric portability backend family id.
    #[error("Tassadar environment spec includes an empty numeric portability backend family id")]
    InvalidNumericPortabilityBackendFamilyId,
    /// Missing numeric portability toolchain families.
    #[error(
        "Tassadar environment spec is missing `numeric_portability_binding.toolchain_family_ids`"
    )]
    MissingNumericPortabilityToolchainFamilyIds,
    /// Invalid numeric portability toolchain family id.
    #[error("Tassadar environment spec includes an empty numeric portability toolchain family id")]
    InvalidNumericPortabilityToolchainFamilyId,
    /// Missing numeric portability profile ids.
    #[error("Tassadar environment spec is missing `numeric_portability_binding.profile_ids`")]
    MissingNumericPortabilityProfileIds,
    /// Invalid numeric portability profile id.
    #[error("Tassadar environment spec includes an empty numeric portability profile id")]
    InvalidNumericPortabilityProfileId,
    /// Missing float-semantics report ref.
    #[error("Tassadar environment spec is missing `float_semantics_binding.report_ref`")]
    MissingFloatSemanticsReportRef,
    /// Missing float-semantics report id.
    #[error("Tassadar environment spec is missing `float_semantics_binding.report_id`")]
    MissingFloatSemanticsReportId,
    /// Missing float-semantics family id.
    #[error("Tassadar environment spec is missing `float_semantics_binding.family_id`")]
    MissingFloatSemanticsFamilyId,
    /// Missing float-semantics profile id.
    #[error("Tassadar environment spec is missing `float_semantics_binding.profile_id`")]
    MissingFloatSemanticsProfileId,
    /// Missing float-semantics NaN policy id.
    #[error("Tassadar environment spec is missing `float_semantics_binding.nan_policy_id`")]
    MissingFloatSemanticsNanPolicyId,
    /// Missing float-semantics canonical NaN bits.
    #[error(
        "Tassadar environment spec is missing `float_semantics_binding.canonical_nan_bits_hex`"
    )]
    MissingFloatSemanticsCanonicalNanBits,
    /// Missing float-semantics comparison policy id.
    #[error("Tassadar environment spec is missing `float_semantics_binding.comparison_policy_id`")]
    MissingFloatSemanticsComparisonPolicyId,
    /// Missing float-semantics refused regimes.
    #[error("Tassadar environment spec must declare `float_semantics_binding.refused_regime_ids`")]
    MissingFloatSemanticsRefusedRegimes,
    /// Invalid float-semantics refused regime id.
    #[error("Tassadar environment spec includes an empty float-semantics refused regime id")]
    InvalidFloatSemanticsRefusedRegimeId,
    /// Missing architecture-bakeoff suite ref.
    #[error("Tassadar environment spec is missing `architecture_bakeoff_binding.suite_ref`")]
    MissingArchitectureBakeoffSuiteRef,
    /// Missing architecture-bakeoff suite version.
    #[error("Tassadar environment spec is missing `architecture_bakeoff_binding.suite_version`")]
    MissingArchitectureBakeoffSuiteVersion,
    /// Missing architecture-bakeoff workload coverage.
    #[error(
        "Tassadar environment spec is missing `architecture_bakeoff_binding.workload_family_ids`"
    )]
    MissingArchitectureBakeoffWorkloadFamilies,
    /// Invalid architecture-bakeoff workload family id.
    #[error("Tassadar environment spec includes an empty architecture-bakeoff workload family id")]
    InvalidArchitectureBakeoffWorkloadFamilyId,
    /// Missing architecture-bakeoff report ref.
    #[error("Tassadar environment spec is missing `architecture_bakeoff_binding.report_ref`")]
    MissingArchitectureBakeoffReportRef,
    /// Missing architecture-bakeoff summary report ref.
    #[error(
        "Tassadar environment spec is missing `architecture_bakeoff_binding.summary_report_ref`"
    )]
    MissingArchitectureBakeoffSummaryReportRef,
    /// Missing Wasm conformance window report ref.
    #[error("Tassadar environment spec is missing `wasm_conformance_binding.window_report_ref`")]
    MissingWasmConformanceWindowReportRef,
    /// Missing Wasm conformance window report id.
    #[error("Tassadar environment spec is missing `wasm_conformance_binding.window_report_id`")]
    MissingWasmConformanceWindowReportId,
    /// Missing Wasm conformance window id.
    #[error("Tassadar environment spec is missing `wasm_conformance_binding.window_id`")]
    MissingWasmConformanceWindowId,
    /// Missing Wasm conformance official harness id.
    #[error("Tassadar environment spec is missing `wasm_conformance_binding.official_harness_id`")]
    MissingWasmConformanceOfficialHarnessId,
    /// Missing Wasm conformance text authority id.
    #[error("Tassadar environment spec is missing `wasm_conformance_binding.text_authority_id`")]
    MissingWasmConformanceTextAuthorityId,
    /// Missing Wasm conformance binary decode authority id.
    #[error(
        "Tassadar environment spec is missing `wasm_conformance_binding.binary_decode_authority_id`"
    )]
    MissingWasmConformanceBinaryDecodeAuthorityId,
    /// Missing Wasm conformance binary encode authority id.
    #[error(
        "Tassadar environment spec is missing `wasm_conformance_binding.binary_encode_authority_id`"
    )]
    MissingWasmConformanceBinaryEncodeAuthorityId,
    /// Missing Wasm conformance validation authority id.
    #[error(
        "Tassadar environment spec is missing `wasm_conformance_binding.validation_authority_id`"
    )]
    MissingWasmConformanceValidationAuthorityId,
    /// Missing Wasm conformance report ref.
    #[error("Tassadar environment spec is missing `wasm_conformance_binding.report_ref`")]
    MissingWasmConformanceReportRef,
    /// Missing Wasm conformance report id.
    #[error("Tassadar environment spec is missing `wasm_conformance_binding.report_id`")]
    MissingWasmConformanceReportId,
    /// Missing Wasm conformance authority id.
    #[error(
        "Tassadar environment spec is missing `wasm_conformance_binding.reference_authority_id`"
    )]
    MissingWasmConformanceAuthorityId,
    /// Missing Wasm conformance case families.
    #[error("Tassadar environment spec must declare Wasm conformance case families")]
    MissingWasmConformanceCaseFamilies,
    /// Invalid Wasm conformance case family id.
    #[error("Tassadar environment spec includes an empty Wasm conformance case family id")]
    InvalidWasmConformanceCaseFamilyId,
    /// Missing Wasm conformance unsupported proposal families.
    #[error(
        "Tassadar environment spec must declare `wasm_conformance_binding.unsupported_proposal_family_ids`"
    )]
    MissingWasmConformanceUnsupportedProposalFamilies,
    /// Invalid Wasm conformance unsupported proposal family id.
    #[error(
        "Tassadar environment spec includes an empty Wasm conformance unsupported proposal family id"
    )]
    InvalidWasmConformanceUnsupportedProposalFamilyId,
    /// Missing module-scale suite ref.
    #[error("Tassadar environment spec is missing `module_scale_workload_suite_binding.suite_ref`")]
    MissingModuleScaleSuiteRef,
    /// Missing module-scale suite version.
    #[error(
        "Tassadar environment spec is missing `module_scale_workload_suite_binding.suite_version`"
    )]
    MissingModuleScaleSuiteVersion,
    /// Missing module-scale families.
    #[error("Tassadar environment spec must declare module-scale workload families")]
    MissingModuleScaleFamilies,
    /// Missing module-scale evaluation axes.
    #[error("Tassadar environment spec must declare module-scale evaluation axes")]
    MissingModuleScaleEvaluationAxes,
    /// Invalid module-scale evaluation axis.
    #[error("Tassadar environment spec includes an empty module-scale evaluation axis")]
    InvalidModuleScaleEvaluationAxis,
    /// Missing module-scale report ref.
    #[error(
        "Tassadar environment spec is missing `module_scale_workload_suite_binding.report_ref`"
    )]
    MissingModuleScaleReportRef,
    /// Missing CLRS-to-Wasm bridge ref.
    #[error("Tassadar environment spec is missing `clrs_wasm_bridge_binding.bridge_ref`")]
    MissingClrsWasmBridgeRef,
    /// Missing CLRS-to-Wasm bridge version.
    #[error("Tassadar environment spec is missing `clrs_wasm_bridge_binding.bridge_version`")]
    MissingClrsWasmBridgeVersion,
    /// Missing CLRS-to-Wasm bridge algorithms.
    #[error("Tassadar environment spec must declare CLRS-to-Wasm bridge algorithms")]
    MissingClrsWasmBridgeAlgorithms,
    /// Missing CLRS-to-Wasm bridge trajectory families.
    #[error("Tassadar environment spec must declare CLRS-to-Wasm bridge trajectory families")]
    MissingClrsWasmBridgeTrajectories,
    /// Missing CLRS-to-Wasm bridge length buckets.
    #[error("Tassadar environment spec must declare CLRS-to-Wasm bridge length buckets")]
    MissingClrsWasmBridgeLengthBuckets,
    /// Missing CLRS-to-Wasm bridge report ref.
    #[error("Tassadar environment spec is missing `clrs_wasm_bridge_binding.report_ref`")]
    MissingClrsWasmBridgeReportRef,
    /// Missing universality witness-suite ref.
    #[error(
        "Tassadar environment spec is missing `universality_witness_suite_binding.suite_ref`"
    )]
    MissingUniversalityWitnessSuiteRef,
    /// Missing universality witness-suite version.
    #[error(
        "Tassadar environment spec is missing `universality_witness_suite_binding.suite_version`"
    )]
    MissingUniversalityWitnessSuiteVersion,
    /// Missing universality witness exact families.
    #[error("Tassadar environment spec must declare universality witness exact families")]
    MissingUniversalityWitnessExactFamilies,
    /// Missing universality witness refusal families.
    #[error("Tassadar environment spec must declare universality witness refusal families")]
    MissingUniversalityWitnessRefusalFamilies,
    /// Missing universality witness evaluation axes.
    #[error("Tassadar environment spec must declare universality witness evaluation axes")]
    MissingUniversalityWitnessEvaluationAxes,
    /// Invalid universality witness evaluation axis.
    #[error("Tassadar environment spec includes an empty universality witness evaluation axis")]
    InvalidUniversalityWitnessEvaluationAxis,
    /// Missing universality witness report ref.
    #[error(
        "Tassadar environment spec is missing `universality_witness_suite_binding.report_ref`"
    )]
    MissingUniversalityWitnessReportRef,
    /// Missing universality witness summary ref.
    #[error(
        "Tassadar environment spec is missing `universality_witness_suite_binding.summary_report_ref`"
    )]
    MissingUniversalityWitnessSummaryRef,
    /// Missing group ref.
    #[error("Tassadar environment refs are missing `group_ref`")]
    MissingGroupRef,
    /// Missing eval pin alias.
    #[error("Tassadar environment refs are missing `eval_pin_alias`")]
    MissingEvalPinAlias,
    /// Missing benchmark pin alias.
    #[error("Tassadar environment refs are missing `benchmark_pin_alias`")]
    MissingBenchmarkPinAlias,
    /// Missing eval member ref.
    #[error("Tassadar environment refs are missing `eval_member_ref`")]
    MissingEvalMemberRef,
    /// Missing benchmark member ref.
    #[error("Tassadar environment refs are missing `benchmark_member_ref`")]
    MissingBenchmarkMemberRef,
    /// Missing program corpus ref.
    #[error("Tassadar environment refs are missing `program_corpus_ref`")]
    MissingProgramCorpusRef,
    /// Missing IO-contract ref.
    #[error("Tassadar environment refs are missing `io_contract_ref`")]
    MissingIoContractRef,
    /// Missing rubric-binding ref.
    #[error("Tassadar environment refs are missing `rubric_binding_ref`")]
    MissingRubricBindingRef,
    /// Missing eval runtime-profile ref.
    #[error("Tassadar environment refs are missing `eval_runtime_profile_ref`")]
    MissingEvalRuntimeProfileRef,
    /// Missing benchmark profile ref.
    #[error("Tassadar environment refs are missing `benchmark_profile_ref`")]
    MissingBenchmarkProfileRef,
    /// Missing benchmark runtime-profile ref.
    #[error("Tassadar environment refs are missing `benchmark_runtime_profile_ref`")]
    MissingBenchmarkRuntimeProfileRef,
    /// Missing dataset ref.
    #[error("Tassadar program binding is missing `dataset.dataset_ref`")]
    MissingDatasetRef,
    /// Missing dataset version.
    #[error("Tassadar program binding is missing `dataset.version`")]
    MissingDatasetVersion,
    /// Missing corpus digest.
    #[error("Tassadar program binding is missing `corpus_digest`")]
    MissingCorpusDigest,
    /// Missing Wasm profile id.
    #[error("Tassadar program binding is missing `wasm_profile_id`")]
    MissingWasmProfileId,
    /// Missing trace ABI id.
    #[error("Tassadar program binding is missing `trace_abi_id`")]
    MissingTraceAbiId,
    /// Invalid trace ABI version.
    #[error("Tassadar program binding requires `trace_abi_version > 0`")]
    InvalidTraceAbiVersion,
    /// Missing opcode-vocabulary digest.
    #[error("Tassadar program binding is missing `opcode_vocabulary_digest`")]
    MissingOpcodeVocabularyDigest,
    /// Missing artifact digests.
    #[error("Tassadar program binding requires at least one artifact digest")]
    MissingArtifactDigests,
    /// Invalid artifact digest.
    #[error("Tassadar program binding includes an empty artifact digest")]
    InvalidArtifactDigest,
    /// Missing input family.
    #[error("Tassadar IO contract is missing `input_family`")]
    MissingInputFamily,
    /// Missing output family.
    #[error("Tassadar IO contract is missing `output_family`")]
    MissingOutputFamily,
    /// Missing output element type.
    #[error("Tassadar IO contract is missing `output_element_type`")]
    MissingOutputElementType,
    /// Final output exactness must be required.
    #[error("Tassadar exactness contract must require final-output exactness")]
    FinalOutputExactnessRequired,
    /// Step exactness must be required.
    #[error("Tassadar exactness contract must require step exactness")]
    StepExactnessRequired,
    /// Halt exactness must be required.
    #[error("Tassadar exactness contract must require halt exactness")]
    HaltExactnessRequired,
    /// Timeout budget must be positive.
    #[error("Tassadar exactness contract requires `timeout_budget_ms > 0`")]
    InvalidTimeoutBudget,
    /// Trace budget must be positive.
    #[error("Tassadar exactness contract requires `trace_budget_steps > 0`")]
    InvalidTraceBudget,
    /// CPU baseline must be required.
    #[error("Tassadar exactness contract must require the direct CPU baseline")]
    CpuBaselineRequired,
    /// Linear reference baseline must be required.
    #[error("Tassadar exactness contract must require the reference-linear baseline")]
    ReferenceLinearBaselineRequired,
    /// Missing current workload targets.
    #[error("Tassadar environment spec requires at least one current workload target")]
    MissingCurrentWorkloadTargets,
    /// Missing eval verification policy ref.
    #[error("Tassadar eval package requires a verification policy ref")]
    MissingEvalVerificationPolicyRef,
    /// Missing benchmark policy ref.
    #[error("Tassadar benchmark package requires a benchmark policy ref")]
    MissingBenchmarkPolicyRef,
    /// Missing benchmark verification policy ref.
    #[error("Tassadar benchmark package requires a verification policy ref")]
    MissingBenchmarkVerificationPolicyRef,
    /// Underlying environment package contract failed.
    #[error(transparent)]
    Contract(#[from] EnvironmentContractError),
    /// Underlying registry composition failed.
    #[error(transparent)]
    Registry(#[from] EnvironmentRegistryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec() -> TassadarEnvironmentSpec {
        TassadarEnvironmentSpec {
            version: String::from("2026.03.15"),
            display_name: String::from("Tassadar Validation Corpus"),
            eval_environment_ref: String::from("env.openagents.tassadar.eval"),
            benchmark_environment_ref: String::from("env.openagents.tassadar.benchmark"),
            eval_dataset: EnvironmentDatasetBinding {
                dataset: DatasetKey::new(
                    "dataset://openagents/tassadar/validation_corpus",
                    "2026.03.15",
                ),
                split: Some(String::from("validation")),
                mount_path: String::from("/datasets/tassadar/validation"),
                required: true,
            },
            benchmark_dataset: EnvironmentDatasetBinding {
                dataset: DatasetKey::new(
                    "dataset://openagents/tassadar/validation_corpus",
                    "2026.03.15",
                ),
                split: Some(String::from("benchmark")),
                mount_path: String::from("/datasets/tassadar/benchmark"),
                required: true,
            },
            package_refs: TassadarEnvironmentPackageRefs {
                group_ref: String::from("group.tassadar.validation"),
                eval_pin_alias: String::from("tassadar_eval"),
                benchmark_pin_alias: String::from("tassadar_benchmark"),
                eval_member_ref: String::from("tassadar_eval_member"),
                benchmark_member_ref: String::from("tassadar_benchmark_member"),
                program_corpus_ref: String::from("tassadar://corpus/phase1.validation"),
                io_contract_ref: String::from("tassadar://io/exact_i32_sequence"),
                rubric_binding_ref: String::from("tassadar://rubric/exactness"),
                eval_runtime_profile_ref: String::from("runtime://tassadar/eval"),
                benchmark_profile_ref: String::from("benchmark://tassadar/reference_fixture"),
                benchmark_runtime_profile_ref: String::from("runtime://tassadar/benchmark"),
            },
            program_binding: TassadarProgramBinding {
                dataset: DatasetKey::new(
                    "dataset://openagents/tassadar/validation_corpus",
                    "2026.03.15",
                ),
                program_corpus_ref: String::from("tassadar://corpus/phase1.validation"),
                corpus_digest: String::from("tassadar-corpus-digest"),
                wasm_profile_id: String::from("tassadar.wasm.core_i32.v1"),
                trace_abi_id: String::from("tassadar.trace.core_i32.v1"),
                trace_abi_version: 1,
                opcode_vocabulary_digest: String::from("opcode-digest"),
                artifact_digests: vec![
                    String::from("artifact-a"),
                    String::from("artifact-b"),
                    String::from("artifact-c"),
                ],
            },
            io_contract: TassadarIoContract::exact_i32_sequence(),
            exactness_contract: TassadarExactnessContract {
                require_final_output_exactness: true,
                require_step_exactness: true,
                require_halt_exactness: true,
                timeout_budget_ms: 5_000,
                trace_budget_steps: 128,
                require_cpu_reference_baseline: true,
                require_reference_linear_baseline: true,
                future_throughput_metric_ids: vec![String::from(
                    "tassadar.hull_cache_steps_per_second",
                )],
            },
            benchmark_package_set_binding: TassadarBenchmarkPackageSetBinding {
                package_set_ref: String::from("benchmark-set://openagents/tassadar/public"),
                package_set_version: String::from("2026.03.17"),
                supported_families: vec![
                    TassadarBenchmarkFamily::Arithmetic,
                    TassadarBenchmarkFamily::ClrsSubset,
                ],
                axis_coverage: vec![
                    TassadarBenchmarkAxis::Exactness,
                    TassadarBenchmarkAxis::LengthGeneralization,
                    TassadarBenchmarkAxis::PlannerUsefulness,
                ],
                summary_report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_benchmark_package_set_summary.json",
                ),
            },
            compile_pipeline_matrix_binding: TassadarCompilePipelineMatrixBinding {
                report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_compile_pipeline_matrix_report.json",
                ),
                report_id: String::from("tassadar.compile_pipeline_matrix_report.v1"),
                source_family_ids: vec![
                    String::from("wasm_text.multi_export_arithmetic"),
                    String::from("wasm_text.memory_lookup"),
                    String::from("wasm_text.param_abi"),
                    String::from("c_source.toolchain_unavailable"),
                ],
            },
            execution_checkpoint_binding: TassadarExecutionCheckpointBinding {
                report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                ),
                report_id: String::from("tassadar.execution_checkpoint.report.v1"),
                run_bundle_ref: String::from(
                    "fixtures/tassadar/runs/tassadar_execution_checkpoint_v1/tassadar_execution_checkpoint_bundle.json",
                ),
                checkpoint_family_id: String::from("tassadar.execution_checkpoint.v1"),
                workload_family_ids: vec![
                    String::from("long_loop_kernel"),
                    String::from("state_machine_accumulator"),
                    String::from("search_frontier_kernel"),
                ],
            },
            process_object_binding: default_tassadar_process_object_binding(),
            spill_tape_store_binding: default_tassadar_spill_tape_store_binding(),
            dynamic_memory_resume_binding: default_tassadar_dynamic_memory_resume_binding(),
            simulator_effect_profile_binding: default_tassadar_simulator_effect_profile_binding(),
            memory64_profile_binding: default_tassadar_memory64_profile_binding(),
            broad_internal_compute_portability_binding: Some(
                default_tassadar_broad_internal_compute_portability_binding(),
            ),
            numeric_portability_binding: Some(default_tassadar_numeric_portability_binding()),
            float_semantics_binding: default_tassadar_float_semantics_binding(),
            wasm_conformance_binding: TassadarWasmConformanceBinding {
                window_report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json",
                ),
                window_report_id: String::from("tassadar.frozen_core_wasm_window.report.v1"),
                window_id: String::from("tassadar.frozen_core_wasm.window.v1"),
                official_harness_id: String::from("tassadar.frozen_core_wasm.harness.v1"),
                text_authority_id: String::from("wat.reference.v1"),
                binary_decode_authority_id: String::from("wasmparser.decode.0.244.0"),
                binary_encode_authority_id: String::from("wasm_encoder.encode.0.244.0"),
                validation_authority_id: String::from("wasmparser.validate.core_int_first.v1"),
                report_ref: String::from(
                    "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json",
                ),
                report_id: String::from("tassadar.wasm_conformance.report.v1"),
                reference_authority_id: String::from("wasmi.reference.v1"),
                case_family_ids: vec![
                    String::from("curated.global_state"),
                    String::from("curated.call_indirect"),
                    String::from("curated.deterministic_import"),
                    String::from("curated.call_indirect_trap"),
                    String::from("curated.unsupported_host_import"),
                    String::from("generated.call_indirect"),
                    String::from("generated.call_indirect_trap"),
                    String::from("generated.global_state"),
                ],
                unsupported_proposal_family_ids: vec![
                    String::from("component_model"),
                    String::from("exceptions"),
                    String::from("floating_point"),
                    String::from("function_references"),
                    String::from("gc"),
                    String::from("memory64"),
                    String::from("multi_memory"),
                    String::from("multi_value"),
                    String::from("relaxed_simd"),
                    String::from("saturating_float_to_int"),
                    String::from("simd"),
                    String::from("tail_call"),
                    String::from("threads"),
                ],
            },
            architecture_bakeoff_binding: Some(default_tassadar_architecture_bakeoff_binding()),
            module_scale_workload_suite_binding: None,
            clrs_wasm_bridge_binding: None,
            eval_policy_references: vec![EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Verification,
                policy_ref: String::from("policy://tassadar/eval/verification"),
                required: true,
            }],
            benchmark_policy_references: vec![
                EnvironmentPolicyReference {
                    kind: EnvironmentPolicyKind::Benchmark,
                    policy_ref: String::from("policy://tassadar/benchmark"),
                    required: true,
                },
                EnvironmentPolicyReference {
                    kind: EnvironmentPolicyKind::Verification,
                    policy_ref: String::from("policy://tassadar/benchmark/verification"),
                    required: true,
                },
            ],
            current_workload_targets: vec![
                TassadarWorkloadTarget::ArithmeticMicroprogram,
                TassadarWorkloadTarget::ClrsShortestPath,
                TassadarWorkloadTarget::MemoryLookupMicroprogram,
                TassadarWorkloadTarget::BranchControlFlowMicroprogram,
            ],
            planned_workload_targets: vec![
                TassadarWorkloadTarget::MicroWasmKernel,
                TassadarWorkloadTarget::SudokuClass,
                TassadarWorkloadTarget::HungarianMatching,
            ],
        }
    }

    #[test]
    fn tassadar_environment_bundle_is_machine_legible() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = sample_spec().build_bundle()?;
        assert_eq!(bundle.eval_resolution.members.len(), 1);
        assert_eq!(bundle.benchmark_resolution.members.len(), 1);
        assert_eq!(
            bundle
                .benchmark_package
                .benchmark_profiles
                .first()
                .map(|profile| profile.benchmark_profile_ref.as_str()),
            Some("benchmark://tassadar/reference_fixture")
        );
        assert_eq!(
            bundle
                .eval_package
                .metadata
                .get(TASSADAR_METADATA_SURFACE_KEY)
                .and_then(Value::as_str),
            Some(TASSADAR_EVAL_METADATA_SURFACE)
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_CURRENT_TARGETS_KEY)
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(4)
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_BENCHMARK_PACKAGE_SET_KEY)
                .and_then(|value| value.get("summary_report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_benchmark_package_set_summary.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_COMPILE_PIPELINE_MATRIX_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_compile_pipeline_matrix_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_RUST_ARTICLE_PROFILE_KEY)
                .and_then(|value| value.get("family_id"))
                .and_then(Value::as_str),
            Some("tassadar.wasm.rust_article_family.v1")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_GENERALIZED_ABI_FAMILY_KEY)
                .and_then(|value| value.get("family_id"))
                .and_then(Value::as_str),
            Some("tassadar.rust_generalized_abi.v1")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_EXECUTION_CHECKPOINT_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_PROCESS_OBJECT_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_process_object_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_DYNAMIC_MEMORY_RESUME_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_SIMULATOR_EFFECT_PROFILE_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_simulator_effect_profile_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_MEMORY64_PROFILE_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_memory64_profile_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_INTERNAL_COMPUTE_PROFILE_LADDER_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_internal_compute_profile_ladder_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_INTERNAL_COMPUTE_PROFILE_CLAIM_KEY)
                .and_then(|value| value.get("claim"))
                .and_then(|value| value.get("profile_id"))
                .and_then(Value::as_str),
            Some("tassadar.internal_compute.article_closeout.v1")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_BROAD_INTERNAL_COMPUTE_PORTABILITY_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some(
                "fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json"
            )
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_NUMERIC_PORTABILITY_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_numeric_portability_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_FLOAT_SEMANTICS_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some(
                "fixtures/tassadar/reports/tassadar_float_semantics_comparison_matrix_report.json"
            )
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_WASM_CONFORMANCE_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_wasm_conformance_report.json")
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_ARCHITECTURE_BAKEOFF_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json")
        );
        assert_eq!(
            bundle.current_workload_targets,
            vec![
                TassadarWorkloadTarget::ArithmeticMicroprogram,
                TassadarWorkloadTarget::ClrsShortestPath,
                TassadarWorkloadTarget::MemoryLookupMicroprogram,
                TassadarWorkloadTarget::BranchControlFlowMicroprogram,
            ]
        );
        assert_eq!(
            bundle.rust_article_profile_completeness.report_ref,
            "fixtures/tassadar/reports/tassadar_rust_article_profile_completeness_report.json"
        );
        assert_eq!(
            bundle.internal_compute_profile_ladder.report_ref,
            "fixtures/tassadar/reports/tassadar_internal_compute_profile_ladder_report.json"
        );
        assert_eq!(
            bundle.generalized_abi_family.report_ref,
            "fixtures/tassadar/reports/tassadar_generalized_abi_family_report.json"
        );
        assert_eq!(
            bundle.float_semantics_binding.report_ref,
            "fixtures/tassadar/reports/tassadar_float_semantics_comparison_matrix_report.json"
        );
        assert_eq!(
            bundle.execution_checkpoint_binding.run_bundle_ref,
            "fixtures/tassadar/runs/tassadar_execution_checkpoint_v1/tassadar_execution_checkpoint_bundle.json"
        );
        assert_eq!(
            bundle.process_object_binding.run_bundle_ref,
            "fixtures/tassadar/runs/tassadar_process_objects_v1/tassadar_process_object_bundle.json"
        );
        assert_eq!(bundle.process_object_binding.process_ids.len(), 3);
        assert_eq!(
            bundle.dynamic_memory_resume_binding.run_bundle_ref,
            "fixtures/tassadar/runs/tassadar_dynamic_memory_resume_v1/tassadar_dynamic_memory_resume_bundle.json"
        );
        assert_eq!(
            bundle.simulator_effect_profile_binding.run_bundle_ref,
            "fixtures/tassadar/runs/tassadar_simulator_effects_v1/tassadar_simulator_effect_runtime_bundle.json"
        );
        assert_eq!(
            bundle.simulator_effect_profile_binding.simulator_profile_ids.len(),
            3
        );
        assert_eq!(
            bundle
                .broad_internal_compute_portability_binding
                .as_ref()
                .map(|binding| binding.profile_ids.len()),
            Some(6)
        );
        assert_eq!(
            bundle
                .broad_internal_compute_portability_binding
                .as_ref()
                .map(|binding| binding.backend_family_ids.clone()),
            Some(vec![
                String::from("cpu_reference"),
                String::from("cuda_served"),
                String::from("metal_served"),
            ])
        );
        assert_eq!(
            bundle
                .broad_internal_compute_portability_binding
                .as_ref()
                .map(|binding| binding.toolchain_family_ids.clone()),
            Some(vec![
                String::from("rustc:wasm32-unknown-unknown"),
                String::from("rustc:wasm32-unknown-unknown+cuda_served"),
                String::from("rustc:wasm32-unknown-unknown+metal_served"),
            ])
        );
        assert_eq!(
            bundle
                .architecture_bakeoff_binding
                .as_ref()
                .map(|binding| binding.workload_family_ids.len()),
            Some(9)
        );
        assert!(bundle.internal_compute_profile_claim_check.green);
        Ok(())
    }

    #[test]
    fn tassadar_environment_bundle_carries_optional_module_scale_suite_binding()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut spec = sample_spec();
        spec.module_scale_workload_suite_binding = Some(TassadarModuleScaleWorkloadSuiteBinding {
            suite_ref: String::from("benchmark-suite://openagents/tassadar/module_scale"),
            suite_version: String::from("2026.03.17"),
            supported_families: vec![
                TassadarModuleScaleWorkloadFamily::Memcpy,
                TassadarModuleScaleWorkloadFamily::Parsing,
                TassadarModuleScaleWorkloadFamily::Checksum,
                TassadarModuleScaleWorkloadFamily::VmStyle,
            ],
            evaluation_axes: vec![
                String::from("exactness_bps"),
                String::from("total_trace_steps"),
                String::from("cpu_reference_cost_units"),
                String::from("refusal_kind"),
            ],
            report_ref: String::from(
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
            ),
        });

        let bundle = spec.build_bundle()?;
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_MODULE_SCALE_WORKLOAD_SUITE_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json")
        );
        assert_eq!(
            bundle
                .module_scale_workload_suite_binding
                .as_ref()
                .map(|binding| binding.supported_families.len()),
            Some(4)
        );
        Ok(())
    }

    #[test]
    fn tassadar_environment_bundle_carries_optional_clrs_wasm_bridge_binding()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut spec = sample_spec();
        spec.clrs_wasm_bridge_binding = Some(TassadarClrsWasmBridgeBinding {
            bridge_ref: String::from("benchmark-bridge://openagents/tassadar/clrs_wasm"),
            bridge_version: String::from("2026.03.18"),
            supported_algorithms: vec![TassadarClrsAlgorithmFamily::ShortestPath],
            trajectory_families: vec![
                TassadarClrsTrajectoryFamily::SequentialRelaxation,
                TassadarClrsTrajectoryFamily::WavefrontRelaxation,
            ],
            length_buckets: vec![
                TassadarClrsLengthBucket::Tiny,
                TassadarClrsLengthBucket::Small,
            ],
            report_ref: String::from(
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            ),
        });

        let bundle = spec.build_bundle()?;
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(TASSADAR_METADATA_CLRS_WASM_BRIDGE_KEY)
                .and_then(|value| value.get("report_ref"))
                .and_then(Value::as_str),
            Some("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json")
        );
        assert_eq!(
            bundle
                .clrs_wasm_bridge_binding
                .as_ref()
                .map(|binding| binding.trajectory_families.len()),
            Some(2)
        );
        Ok(())
    }

    #[test]
    fn tassadar_environment_spec_requires_benchmark_and_verification_policy_refs() {
        let mut spec = sample_spec();
        spec.benchmark_policy_references.clear();
        let err = spec
            .build_bundle()
            .expect_err("missing benchmark policies should fail");
        assert_eq!(err, TassadarEnvironmentError::MissingBenchmarkPolicyRef);

        let mut spec = sample_spec();
        spec.eval_policy_references.clear();
        let err = spec
            .build_bundle()
            .expect_err("missing eval verification policy should fail");
        assert_eq!(
            err,
            TassadarEnvironmentError::MissingEvalVerificationPolicyRef
        );
    }
}
