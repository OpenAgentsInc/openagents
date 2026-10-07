use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest;
use thiserror::Error;

use crate::{
    EnvironmentArtifactExpectation, EnvironmentBenchmarkProfile, EnvironmentCompositionGroup,
    EnvironmentCompositionMember, EnvironmentContractError, EnvironmentDatasetBinding,
    EnvironmentDifficultyMetadata, EnvironmentEvalParityReceipt, EnvironmentExecutionEntrypoint,
    EnvironmentGroupResolution, EnvironmentPackageContract, EnvironmentPackageFamily,
    EnvironmentPackageInstallSource, EnvironmentPackageKey, EnvironmentPolicyKind,
    EnvironmentPolicyReference, EnvironmentRegistry, EnvironmentRegistryError,
    EnvironmentRubricHook, EnvironmentRuntimeFamily, EnvironmentStateMode, EnvironmentToolContract,
    EnvironmentUsageSurface, EnvironmentVerificationPosture, EnvironmentWorkloadClass,
};

const APPLE_ADAPTER_CORE_METADATA_SURFACE: &str = "core";
const APPLE_ADAPTER_BENCHMARK_METADATA_SURFACE: &str = "benchmark";
const APPLE_ADAPTER_METADATA_ABI_VERSION_KEY: &str = "apple_adapter.abi_version";
const APPLE_ADAPTER_METADATA_RUNTIME_REQUIREMENTS_KEY: &str = "apple_adapter.runtime_requirements";
const APPLE_ADAPTER_METADATA_PACKAGE_REFS_KEY: &str = "apple_adapter.package_refs";
const APPLE_ADAPTER_METADATA_SURFACE_KEY: &str = "apple_adapter.surface";

/// Stable ABI version for the typed Apple adapter environment bundle helper.
pub const APPLE_ADAPTER_ENVIRONMENT_ABI_VERSION: &str = "psionic.apple_adapter_environment.v1";

/// Typed package refs that Apple train/eval/benchmark environments reuse.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterEnvironmentPackageRefs {
    /// Stable environment-group reference reused across train/eval/benchmark.
    pub group_ref: String,
    /// Pin alias for the shared train/eval package.
    pub core_pin_alias: String,
    /// Pin alias for the benchmark package.
    pub benchmark_pin_alias: String,
    /// Member ref reused across the train and eval surfaces.
    pub core_member_ref: String,
    /// Member ref used only for the benchmark surface.
    pub benchmark_member_ref: String,
    /// Stable session-profile ref for Apple dialog sessions.
    pub session_profile_ref: String,
    /// Stable runtime-profile ref for the Apple bridge/runtime lane.
    pub runtime_profile_ref: String,
    /// Stable bundle ref for the tool contract set.
    pub tool_bundle_ref: String,
    /// Stable bundle ref for rubric bindings.
    pub rubric_binding_ref: String,
    /// Optional stable structured-output profile ref.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output_profile_ref: Option<String>,
    /// Stable benchmark profile ref for validator and held-out reuse.
    pub benchmark_profile_ref: String,
    /// Stable benchmark runtime-profile ref.
    pub benchmark_runtime_profile_ref: String,
}

impl AppleAdapterEnvironmentPackageRefs {
    /// Returns a stable digest over the package refs.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("Apple package refs should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the package refs are present.
    pub fn validate(&self) -> Result<(), AppleAdapterEnvironmentError> {
        if self.group_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingGroupRef);
        }
        if self.core_pin_alias.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingCorePinAlias);
        }
        if self.benchmark_pin_alias.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingBenchmarkPinAlias);
        }
        if self.core_member_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingCoreMemberRef);
        }
        if self.benchmark_member_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingBenchmarkMemberRef);
        }
        if self.session_profile_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingSessionProfileRef);
        }
        if self.runtime_profile_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingRuntimeProfileRef);
        }
        if self.tool_bundle_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingToolBundleRef);
        }
        if self.rubric_binding_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingRubricBindingRef);
        }
        if self.benchmark_profile_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingBenchmarkProfileRef);
        }
        if self.benchmark_runtime_profile_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingBenchmarkRuntimeProfileRef);
        }
        Ok(())
    }
}

/// Typed Apple runtime/session capability requirements for train/eval parity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterEnvironmentRuntimeRequirements {
    /// Stable bridge reference or lane id.
    pub foundation_bridge_ref: String,
    /// Stable Apple model id.
    pub model_id: String,
    /// Minimum platform or host requirement string.
    pub platform_requirement: String,
    /// Whether adapter inventory support is required.
    pub adapter_inventory_required: bool,
    /// Whether session attach/detach support is required.
    pub session_attach_required: bool,
    /// Whether the environment assumes structured-output support.
    pub structured_output_supported: bool,
    /// Whether the environment assumes tool-calling support.
    pub tool_calling_supported: bool,
    /// Maximum context window expected by the package family.
    pub max_context_tokens: u32,
    /// Maximum dialog turns allowed in the shared train/eval package.
    pub max_session_turns: u32,
    /// Time budget for one runtime session.
    pub time_budget_ms: u64,
}

impl AppleAdapterEnvironmentRuntimeRequirements {
    /// Returns a stable digest over the runtime requirements.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("Apple runtime requirements should serialize");
        let digest = sha2::Sha256::digest(encoded.as_slice());
        hex::encode(digest)
    }

    /// Validates that the runtime requirements are explicit.
    pub fn validate(
        &self,
        package_refs: &AppleAdapterEnvironmentPackageRefs,
    ) -> Result<(), AppleAdapterEnvironmentError> {
        if self.foundation_bridge_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingFoundationBridgeRef);
        }
        if self.model_id.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingModelId);
        }
        if self.platform_requirement.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingPlatformRequirement);
        }
        if self.max_context_tokens == 0 {
            return Err(AppleAdapterEnvironmentError::InvalidMaxContextTokens);
        }
        if self.max_session_turns == 0 {
            return Err(AppleAdapterEnvironmentError::InvalidMaxSessionTurns);
        }
        if self.time_budget_ms == 0 {
            return Err(AppleAdapterEnvironmentError::InvalidTimeBudget);
        }
        if self.structured_output_supported
            && package_refs
                .structured_output_profile_ref
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            return Err(AppleAdapterEnvironmentError::MissingStructuredOutputProfileRef);
        }
        Ok(())
    }
}

/// Builder input for a reusable Apple adapter train/eval/benchmark environment bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterEnvironmentSpec {
    /// Immutable package version shared across the bundle.
    pub version: String,
    /// Shared display label.
    pub display_name: String,
    /// Environment ref for the shared train/eval package.
    pub core_environment_ref: String,
    /// Environment ref for the benchmark package.
    pub benchmark_environment_ref: String,
    /// Train split binding.
    pub train_dataset: EnvironmentDatasetBinding,
    /// Held-out eval split binding reused with the same core package.
    pub held_out_eval_dataset: EnvironmentDatasetBinding,
    /// Optional benchmark dataset binding; defaults to the held-out eval binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_dataset: Option<EnvironmentDatasetBinding>,
    /// Typed package refs reused across the bundle.
    pub package_refs: AppleAdapterEnvironmentPackageRefs,
    /// Runtime/session capability requirements.
    pub runtime_requirements: AppleAdapterEnvironmentRuntimeRequirements,
    /// Tool bundle surfaced to train/eval/benchmark.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<EnvironmentToolContract>,
    /// Rubric hooks surfaced to train/eval/benchmark.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rubric_hooks: Vec<EnvironmentRubricHook>,
    /// Expected artifacts for the core package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_artifacts: Vec<EnvironmentArtifactExpectation>,
    /// Policy refs for the shared train/eval package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub core_policy_references: Vec<EnvironmentPolicyReference>,
    /// Policy refs for the benchmark package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub benchmark_policy_references: Vec<EnvironmentPolicyReference>,
    /// Optional difficulty metadata for the shared package.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub difficulty: Option<EnvironmentDifficultyMetadata>,
}

impl AppleAdapterEnvironmentSpec {
    /// Builds the reusable Apple adapter environment bundle and proves train/eval parity.
    pub fn build_bundle(
        &self,
    ) -> Result<AppleAdapterEnvironmentBundle, AppleAdapterEnvironmentError> {
        self.validate()?;
        let core_package = self.core_package()?;
        let benchmark_package = self.benchmark_package()?;
        let group = self.group_definition();

        let mut registry = EnvironmentRegistry::default();
        registry
            .install_package(crate::EnvironmentInstallRequest {
                package: core_package.clone(),
                source: EnvironmentPackageInstallSource::BuiltIn {
                    owner: String::from("apple_adapter_environment_bundle"),
                },
                dependencies: Vec::new(),
            })
            .map_err(AppleAdapterEnvironmentError::Registry)?;
        registry
            .install_package(crate::EnvironmentInstallRequest {
                package: benchmark_package.clone(),
                source: EnvironmentPackageInstallSource::BuiltIn {
                    owner: String::from("apple_adapter_environment_bundle"),
                },
                dependencies: vec![core_package.key.clone()],
            })
            .map_err(AppleAdapterEnvironmentError::Registry)?;
        registry
            .pin_package(
                self.package_refs.core_pin_alias.clone(),
                core_package.key.clone(),
                vec![
                    EnvironmentWorkloadClass::Sft,
                    EnvironmentWorkloadClass::OnlineEval,
                    EnvironmentWorkloadClass::OfflineEval,
                ],
            )
            .map_err(AppleAdapterEnvironmentError::Registry)?;
        registry
            .pin_package(
                self.package_refs.benchmark_pin_alias.clone(),
                benchmark_package.key.clone(),
                vec![EnvironmentWorkloadClass::ValidatorBenchmark],
            )
            .map_err(AppleAdapterEnvironmentError::Registry)?;
        registry
            .define_group(group.clone())
            .map_err(AppleAdapterEnvironmentError::Registry)?;

        let train_resolution = registry
            .resolve_group(
                self.package_refs.group_ref.as_str(),
                EnvironmentUsageSurface::Train,
            )
            .map_err(AppleAdapterEnvironmentError::Registry)?;
        let eval_resolution = registry
            .resolve_group(
                self.package_refs.group_ref.as_str(),
                EnvironmentUsageSurface::Eval,
            )
            .map_err(AppleAdapterEnvironmentError::Registry)?;
        let benchmark_resolution = registry
            .resolve_group(
                self.package_refs.group_ref.as_str(),
                EnvironmentUsageSurface::Benchmark,
            )
            .map_err(AppleAdapterEnvironmentError::Registry)?;
        let train_eval_parity = registry
            .verify_eval_parity(self.package_refs.group_ref.as_str())
            .map_err(AppleAdapterEnvironmentError::Registry)?;

        Ok(AppleAdapterEnvironmentBundle {
            core_package,
            benchmark_package,
            group,
            train_resolution,
            eval_resolution,
            benchmark_resolution,
            train_eval_parity,
        })
    }

    fn validate(&self) -> Result<(), AppleAdapterEnvironmentError> {
        if self.version.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingVersion);
        }
        if self.display_name.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingDisplayName);
        }
        if self.core_environment_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingCoreEnvironmentRef);
        }
        if self.benchmark_environment_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingBenchmarkEnvironmentRef);
        }
        if self.core_environment_ref == self.benchmark_environment_ref {
            return Err(AppleAdapterEnvironmentError::DuplicateEnvironmentRef);
        }
        if self.train_dataset.dataset.dataset_ref.trim().is_empty() {
            return Err(AppleAdapterEnvironmentError::MissingTrainDataset);
        }
        if self
            .held_out_eval_dataset
            .dataset
            .dataset_ref
            .trim()
            .is_empty()
        {
            return Err(AppleAdapterEnvironmentError::MissingHeldOutEvalDataset);
        }
        self.package_refs.validate()?;
        self.runtime_requirements.validate(&self.package_refs)?;
        if !self
            .core_policy_references
            .iter()
            .any(|policy| policy.kind == EnvironmentPolicyKind::Training)
        {
            return Err(AppleAdapterEnvironmentError::MissingTrainingPolicyRef);
        }
        if !self
            .benchmark_policy_references
            .iter()
            .any(|policy| policy.kind == EnvironmentPolicyKind::Benchmark)
        {
            return Err(AppleAdapterEnvironmentError::MissingBenchmarkPolicyRef);
        }
        Ok(())
    }

    fn core_package(&self) -> Result<EnvironmentPackageContract, AppleAdapterEnvironmentError> {
        let mut core_package = EnvironmentPackageContract::new(
            EnvironmentPackageKey::new(self.core_environment_ref.clone(), self.version.clone()),
            EnvironmentPackageFamily::Sft,
            format!("{} Core", self.display_name),
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::MultiTurnDialog,
                entrypoint: String::from("apple_adapter::session"),
                args: vec![
                    format!(
                        "--session-profile-ref={}",
                        self.package_refs.session_profile_ref
                    ),
                    format!(
                        "--runtime-profile-ref={}",
                        self.package_refs.runtime_profile_ref
                    ),
                    format!(
                        "--bridge-ref={}",
                        self.runtime_requirements.foundation_bridge_ref
                    ),
                ],
                sandbox_profile_ref: Some(String::from("sandbox.profile.apple_adapter.core")),
                max_turns: self.runtime_requirements.max_session_turns,
                state_mode: EnvironmentStateMode::SessionPersistent,
                time_budget_ms: Some(self.runtime_requirements.time_budget_ms),
            },
        )
        .with_supported_workloads(vec![
            EnvironmentWorkloadClass::Sft,
            EnvironmentWorkloadClass::OnlineEval,
            EnvironmentWorkloadClass::OfflineEval,
        ])
        .with_datasets(vec![
            self.train_dataset.clone(),
            self.held_out_eval_dataset.clone(),
        ])
        .with_tools(self.tools.clone())
        .with_rubric_hooks(self.rubric_hooks.clone())
        .with_expected_artifacts(self.expected_artifacts.clone())
        .with_policy_references(self.core_policy_references.clone())
        .with_metadata(self.package_metadata(APPLE_ADAPTER_CORE_METADATA_SURFACE));
        if let Some(difficulty) = &self.difficulty {
            core_package = core_package.with_difficulty(difficulty.clone());
        }
        core_package
            .validate()
            .map_err(AppleAdapterEnvironmentError::Contract)?;
        Ok(core_package)
    }

    fn benchmark_package(
        &self,
    ) -> Result<EnvironmentPackageContract, AppleAdapterEnvironmentError> {
        let benchmark_dataset = self
            .benchmark_dataset
            .clone()
            .unwrap_or_else(|| self.held_out_eval_dataset.clone());
        let benchmark_package = EnvironmentPackageContract::new(
            EnvironmentPackageKey::new(
                self.benchmark_environment_ref.clone(),
                self.version.clone(),
            ),
            EnvironmentPackageFamily::Evaluation,
            format!("{} Benchmark", self.display_name),
            EnvironmentExecutionEntrypoint {
                runtime_family: EnvironmentRuntimeFamily::Evaluator,
                entrypoint: String::from("apple_adapter::benchmark"),
                args: vec![
                    format!(
                        "--benchmark-profile-ref={}",
                        self.package_refs.benchmark_profile_ref
                    ),
                    format!(
                        "--benchmark-runtime-profile-ref={}",
                        self.package_refs.benchmark_runtime_profile_ref
                    ),
                    format!(
                        "--bridge-ref={}",
                        self.runtime_requirements.foundation_bridge_ref
                    ),
                ],
                sandbox_profile_ref: Some(String::from("sandbox.profile.apple_adapter.benchmark")),
                max_turns: 1,
                state_mode: EnvironmentStateMode::TurnScoped,
                time_budget_ms: Some(self.runtime_requirements.time_budget_ms),
            },
        )
        .with_supported_workloads(vec![
            EnvironmentWorkloadClass::OfflineEval,
            EnvironmentWorkloadClass::ValidatorBenchmark,
        ])
        .with_datasets(vec![benchmark_dataset])
        .with_tools(self.tools.clone())
        .with_rubric_hooks(self.rubric_hooks.clone())
        .with_expected_artifacts(self.expected_artifacts.clone())
        .with_policy_references(self.benchmark_policy_references.clone())
        .with_benchmark_profiles(vec![EnvironmentBenchmarkProfile {
            benchmark_profile_ref: self.package_refs.benchmark_profile_ref.clone(),
            runtime_profile_ref: self.package_refs.benchmark_runtime_profile_ref.clone(),
            verification_posture: EnvironmentVerificationPosture::ValidatorRequired,
            expected_execution_strategy: Some(String::from("apple_foundation_models")),
        }])
        .with_metadata(self.package_metadata(APPLE_ADAPTER_BENCHMARK_METADATA_SURFACE));
        benchmark_package
            .validate()
            .map_err(AppleAdapterEnvironmentError::Contract)?;
        Ok(benchmark_package)
    }

    fn group_definition(&self) -> EnvironmentCompositionGroup {
        EnvironmentCompositionGroup {
            group_ref: self.package_refs.group_ref.clone(),
            display_name: self.display_name.clone(),
            members: vec![
                EnvironmentCompositionMember {
                    member_ref: self.package_refs.core_member_ref.clone(),
                    pin_alias: self.package_refs.core_pin_alias.clone(),
                    surfaces: vec![
                        EnvironmentUsageSurface::Train,
                        EnvironmentUsageSurface::Eval,
                    ],
                    required_workloads: vec![
                        EnvironmentWorkloadClass::Sft,
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

    fn package_metadata(&self, surface: &str) -> BTreeMap<String, Value> {
        let mut metadata = BTreeMap::new();
        metadata.insert(
            String::from(APPLE_ADAPTER_METADATA_ABI_VERSION_KEY),
            Value::String(String::from(APPLE_ADAPTER_ENVIRONMENT_ABI_VERSION)),
        );
        metadata.insert(
            String::from(APPLE_ADAPTER_METADATA_RUNTIME_REQUIREMENTS_KEY),
            serde_json::to_value(&self.runtime_requirements)
                .expect("Apple runtime requirements should serialize"),
        );
        metadata.insert(
            String::from(APPLE_ADAPTER_METADATA_PACKAGE_REFS_KEY),
            serde_json::to_value(&self.package_refs).expect("Apple package refs should serialize"),
        );
        metadata.insert(
            String::from(APPLE_ADAPTER_METADATA_SURFACE_KEY),
            Value::String(String::from(surface)),
        );
        metadata.insert(
            String::from("apple_adapter.bundle_digests"),
            json!({
                "runtime_requirements_digest": self.runtime_requirements.stable_digest(),
                "package_refs_digest": self.package_refs.stable_digest(),
            }),
        );
        metadata
    }
}

/// Resolved reusable Apple adapter environment bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterEnvironmentBundle {
    /// Shared train/eval package.
    pub core_package: EnvironmentPackageContract,
    /// Benchmark-only package.
    pub benchmark_package: EnvironmentPackageContract,
    /// Mixed-surface environment group.
    pub group: EnvironmentCompositionGroup,
    /// Train-surface resolution.
    pub train_resolution: EnvironmentGroupResolution,
    /// Eval-surface resolution.
    pub eval_resolution: EnvironmentGroupResolution,
    /// Benchmark-surface resolution.
    pub benchmark_resolution: EnvironmentGroupResolution,
    /// Explicit parity receipt proving train/eval reuse the same shared package.
    pub train_eval_parity: EnvironmentEvalParityReceipt,
}

/// Apple adapter environment spec/build failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AppleAdapterEnvironmentError {
    /// Missing shared package version.
    #[error("Apple adapter environment spec is missing `version`")]
    MissingVersion,
    /// Missing display name.
    #[error("Apple adapter environment spec is missing `display_name`")]
    MissingDisplayName,
    /// Missing core environment ref.
    #[error("Apple adapter environment spec is missing `core_environment_ref`")]
    MissingCoreEnvironmentRef,
    /// Missing benchmark environment ref.
    #[error("Apple adapter environment spec is missing `benchmark_environment_ref`")]
    MissingBenchmarkEnvironmentRef,
    /// Core and benchmark refs must not be identical.
    #[error("Apple adapter environment spec must use distinct core and benchmark refs")]
    DuplicateEnvironmentRef,
    /// Missing train dataset.
    #[error("Apple adapter environment spec is missing the train dataset binding")]
    MissingTrainDataset,
    /// Missing held-out eval dataset.
    #[error("Apple adapter environment spec is missing the held-out eval dataset binding")]
    MissingHeldOutEvalDataset,
    /// Missing group ref.
    #[error("Apple adapter environment refs are missing `group_ref`")]
    MissingGroupRef,
    /// Missing shared core pin alias.
    #[error("Apple adapter environment refs are missing `core_pin_alias`")]
    MissingCorePinAlias,
    /// Missing benchmark pin alias.
    #[error("Apple adapter environment refs are missing `benchmark_pin_alias`")]
    MissingBenchmarkPinAlias,
    /// Missing shared core member ref.
    #[error("Apple adapter environment refs are missing `core_member_ref`")]
    MissingCoreMemberRef,
    /// Missing benchmark member ref.
    #[error("Apple adapter environment refs are missing `benchmark_member_ref`")]
    MissingBenchmarkMemberRef,
    /// Missing session profile ref.
    #[error("Apple adapter environment refs are missing `session_profile_ref`")]
    MissingSessionProfileRef,
    /// Missing runtime profile ref.
    #[error("Apple adapter environment refs are missing `runtime_profile_ref`")]
    MissingRuntimeProfileRef,
    /// Missing tool bundle ref.
    #[error("Apple adapter environment refs are missing `tool_bundle_ref`")]
    MissingToolBundleRef,
    /// Missing rubric-binding ref.
    #[error("Apple adapter environment refs are missing `rubric_binding_ref`")]
    MissingRubricBindingRef,
    /// Missing structured-output profile ref when structured output is required.
    #[error(
        "Apple adapter environment refs are missing `structured_output_profile_ref` for a structured-output runtime"
    )]
    MissingStructuredOutputProfileRef,
    /// Missing benchmark profile ref.
    #[error("Apple adapter environment refs are missing `benchmark_profile_ref`")]
    MissingBenchmarkProfileRef,
    /// Missing benchmark runtime profile ref.
    #[error("Apple adapter environment refs are missing `benchmark_runtime_profile_ref`")]
    MissingBenchmarkRuntimeProfileRef,
    /// Missing bridge reference.
    #[error("Apple adapter runtime requirements are missing `foundation_bridge_ref`")]
    MissingFoundationBridgeRef,
    /// Missing model id.
    #[error("Apple adapter runtime requirements are missing `model_id`")]
    MissingModelId,
    /// Missing platform requirement.
    #[error("Apple adapter runtime requirements are missing `platform_requirement`")]
    MissingPlatformRequirement,
    /// Invalid context window.
    #[error("Apple adapter runtime requirements require `max_context_tokens > 0`")]
    InvalidMaxContextTokens,
    /// Invalid max-turn budget.
    #[error("Apple adapter runtime requirements require `max_session_turns > 0`")]
    InvalidMaxSessionTurns,
    /// Invalid time budget.
    #[error("Apple adapter runtime requirements require `time_budget_ms > 0`")]
    InvalidTimeBudget,
    /// Missing training policy ref on the core package.
    #[error("Apple adapter environment spec requires a core policy ref with kind `training`")]
    MissingTrainingPolicyRef,
    /// Missing benchmark policy ref on the benchmark package.
    #[error("Apple adapter environment spec requires a benchmark policy ref with kind `benchmark`")]
    MissingBenchmarkPolicyRef,
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
    use psionic_data::DatasetKey;
    use serde_json::json;

    fn sample_spec() -> AppleAdapterEnvironmentSpec {
        AppleAdapterEnvironmentSpec {
            version: String::from("2026.03.15"),
            display_name: String::from("Apple Adapter Helpdesk"),
            core_environment_ref: String::from("env.openagents.apple_adapter.helpdesk.core"),
            benchmark_environment_ref: String::from(
                "env.openagents.apple_adapter.helpdesk.benchmark",
            ),
            train_dataset: EnvironmentDatasetBinding {
                dataset: DatasetKey::new(
                    "dataset://openagents/apple-adapter-helpdesk",
                    "2026.03.15",
                ),
                split: Some(String::from("train")),
                mount_path: String::from("/datasets/apple/train"),
                required: true,
            },
            held_out_eval_dataset: EnvironmentDatasetBinding {
                dataset: DatasetKey::new(
                    "dataset://openagents/apple-adapter-helpdesk",
                    "2026.03.15",
                ),
                split: Some(String::from("held_out")),
                mount_path: String::from("/datasets/apple/held_out"),
                required: true,
            },
            benchmark_dataset: Some(EnvironmentDatasetBinding {
                dataset: DatasetKey::new(
                    "dataset://openagents/apple-adapter-helpdesk",
                    "2026.03.15",
                ),
                split: Some(String::from("benchmark")),
                mount_path: String::from("/datasets/apple/benchmark"),
                required: true,
            }),
            package_refs: AppleAdapterEnvironmentPackageRefs {
                group_ref: String::from("group.apple_adapter.helpdesk"),
                core_pin_alias: String::from("apple_adapter_helpdesk_core"),
                benchmark_pin_alias: String::from("apple_adapter_helpdesk_benchmark"),
                core_member_ref: String::from("apple_core"),
                benchmark_member_ref: String::from("apple_benchmark"),
                session_profile_ref: String::from("session://apple/helpdesk"),
                runtime_profile_ref: String::from("runtime://apple/foundation-models"),
                tool_bundle_ref: String::from("tools://apple/helpdesk"),
                rubric_binding_ref: String::from("rubric://apple/helpdesk"),
                structured_output_profile_ref: Some(String::from(
                    "structured://apple/helpdesk/json",
                )),
                benchmark_profile_ref: String::from("benchmark://apple/helpdesk/default"),
                benchmark_runtime_profile_ref: String::from("runtime://apple/helpdesk/benchmark"),
            },
            runtime_requirements: AppleAdapterEnvironmentRuntimeRequirements {
                foundation_bridge_ref: String::from("bridge://apple-foundation-models"),
                model_id: String::from("apple-foundation-model"),
                platform_requirement: String::from("macos26_apple_silicon"),
                adapter_inventory_required: true,
                session_attach_required: true,
                structured_output_supported: true,
                tool_calling_supported: true,
                max_context_tokens: 4096,
                max_session_turns: 4,
                time_budget_ms: 30_000,
            },
            tools: vec![EnvironmentToolContract {
                tool_name: String::from("lookup_ticket"),
                interface: crate::EnvironmentToolInterface::NativeFunction,
                description: String::from("Looks up one helpdesk ticket."),
                args_schema: json!({
                    "type": "object",
                    "properties": {
                        "ticket_id": {"type": "string"}
                    },
                    "required": ["ticket_id"],
                    "additionalProperties": false
                }),
                result_schema: Some(json!({
                    "type": "object",
                    "properties": {
                        "status": {"type": "string"}
                    },
                    "required": ["status"],
                    "additionalProperties": false
                })),
            }],
            rubric_hooks: vec![EnvironmentRubricHook {
                rubric_ref: String::from("rubric://apple/helpdesk/answer"),
                hook_name: String::from("score_helpdesk_answer"),
                score_kind: crate::EnvironmentRubricScoreKind::Scalar,
                pass_threshold: Some(8500),
            }],
            expected_artifacts: vec![EnvironmentArtifactExpectation {
                artifact_kind: String::from("adapter_trace.json"),
                required: true,
                verification_policy_ref: Some(String::from("verify://apple/helpdesk/trace")),
            }],
            core_policy_references: vec![
                EnvironmentPolicyReference {
                    kind: EnvironmentPolicyKind::Training,
                    policy_ref: String::from("policy://apple/helpdesk/train"),
                    required: true,
                },
                EnvironmentPolicyReference {
                    kind: EnvironmentPolicyKind::Safety,
                    policy_ref: String::from("policy://apple/helpdesk/safety"),
                    required: true,
                },
            ],
            benchmark_policy_references: vec![EnvironmentPolicyReference {
                kind: EnvironmentPolicyKind::Benchmark,
                policy_ref: String::from("policy://apple/helpdesk/benchmark"),
                required: true,
            }],
            difficulty: Some(EnvironmentDifficultyMetadata {
                difficulty_tier: String::from("narrow_domain"),
                min_agent_level: Some(1),
                tags: vec![
                    String::from("apple_adapter"),
                    String::from("structured_output"),
                ],
            }),
        }
    }

    #[test]
    fn apple_adapter_environment_bundle_reuses_same_core_package_for_train_and_eval()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = sample_spec().build_bundle()?;
        assert_eq!(bundle.core_package.family, EnvironmentPackageFamily::Sft);
        assert_eq!(
            bundle.benchmark_package.family,
            EnvironmentPackageFamily::Evaluation
        );
        assert!(
            bundle
                .core_package
                .supported_workloads
                .contains(&EnvironmentWorkloadClass::OfflineEval)
        );
        assert_eq!(bundle.train_resolution.members.len(), 1);
        assert_eq!(bundle.eval_resolution.members.len(), 1);
        assert_eq!(bundle.benchmark_resolution.members.len(), 1);
        assert_eq!(
            bundle.train_resolution.members[0].package.package_key,
            bundle.eval_resolution.members[0].package.package_key
        );
        assert_eq!(
            bundle.train_resolution.members[0].package.package_digest,
            bundle.eval_resolution.members[0].package.package_digest
        );
        assert_eq!(
            bundle.train_eval_parity.reused_member_refs,
            vec![String::from("apple_core")]
        );
        assert_eq!(
            bundle
                .core_package
                .metadata
                .get(APPLE_ADAPTER_METADATA_SURFACE_KEY)
                .and_then(Value::as_str),
            Some(APPLE_ADAPTER_CORE_METADATA_SURFACE)
        );
        assert_eq!(
            bundle
                .benchmark_package
                .metadata
                .get(APPLE_ADAPTER_METADATA_SURFACE_KEY)
                .and_then(Value::as_str),
            Some(APPLE_ADAPTER_BENCHMARK_METADATA_SURFACE)
        );
        assert_eq!(
            bundle
                .core_package
                .metadata
                .get(APPLE_ADAPTER_METADATA_PACKAGE_REFS_KEY)
                .and_then(|value| value.get("tool_bundle_ref"))
                .and_then(Value::as_str),
            Some("tools://apple/helpdesk")
        );
        Ok(())
    }

    #[test]
    fn apple_adapter_environment_spec_requires_explicit_runtime_and_profile_refs() {
        let mut spec = sample_spec();
        spec.package_refs.structured_output_profile_ref = None;
        let err = spec
            .build_bundle()
            .expect_err("missing structured-output ref should fail");
        assert_eq!(
            err,
            AppleAdapterEnvironmentError::MissingStructuredOutputProfileRef
        );

        let mut spec = sample_spec();
        spec.core_policy_references.clear();
        let err = spec
            .build_bundle()
            .expect_err("missing training policy should fail");
        assert_eq!(err, AppleAdapterEnvironmentError::MissingTrainingPolicyRef);
    }
}
