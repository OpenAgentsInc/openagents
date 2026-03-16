use std::collections::BTreeMap;

use psionic_data::DatasetKey;
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
    /// Memory or local read-write microprograms.
    MemoryLookupMicroprogram,
    /// Branch and control-flow microprograms.
    BranchControlFlowMicroprogram,
    /// Richer WebAssembly kernels beyond the current microprogram corpus.
    MicroWasmKernel,
    /// Sudoku-style exact search workloads.
    SudokuClass,
    /// Hungarian or min-cost-matching style workloads.
    HungarianMatching,
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
            Some(3)
        );
        assert_eq!(
            bundle.current_workload_targets,
            vec![
                TassadarWorkloadTarget::ArithmeticMicroprogram,
                TassadarWorkloadTarget::MemoryLookupMicroprogram,
                TassadarWorkloadTarget::BranchControlFlowMicroprogram,
            ]
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
