use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_core::{DType, Device, DeviceKind};
use psionic_eval::{
    build_parameter_golf_submission_promotion_receipt, ParameterGolfArtifactSizeReceipt,
    ParameterGolfChallengeBenchmarkReceipt, ParameterGolfDistributedChallengeThresholds,
    ParameterGolfDistributedCommunicationReceipt, ParameterGolfDistributedLaneDisposition,
    ParameterGolfDistributedLaneRefusal, ParameterGolfDistributedLaneRefusalKind,
    ParameterGolfDistributedThroughputReceipt, ParameterGolfDistributedTopologyReceipt,
    ParameterGolfMemoryReceipt, ParameterGolfSubmissionPromotionCandidate,
    ParameterGolfSubmissionPromotionDisposition, ParameterGolfSubmissionPromotionReceipt,
    ParameterGolfWallclockReceipt, PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF,
    PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF,
    PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY,
};
use psionic_ir::GraphError;
use psionic_models::{ParameterGolfModelError, ParameterGolfReferenceModel};
use psionic_runtime::{
    BackendSelection, BackendSelectionState, ClusterCommunicationClass, ClusterExecutionCapabilityProfile,
    ClusterTransportClass, DeviceDescriptor, ExecutionTopologyPlan, NvidiaDeviceMetadata,
    NvidiaRecoveryAction, NvidiaRecoveryProfile, NvidiaRiskLevel, NvidiaRiskProfile,
    NvidiaTopologyInfo, ServedProductBackendPolicy, ServedProductFallbackTrigger,
};

use crate::{
    benchmark_parameter_golf_local_reference, build_parameter_golf_non_record_submission_bundle,
    builtin_parameter_golf_cuda_training_capability_report,
    canonicalize_parameter_golf_local_reference_benchmark_bundle,
    write_parameter_golf_non_record_submission_bundle, ParameterGolfBenchmarkBundleError,
    ParameterGolfChallengeRunBundle, ParameterGolfLocalReferenceFixture,
    ParameterGolfNonRecordSubmissionConfig, ParameterGolfNonRecordSubmissionManifest,
    ParameterGolfReferenceTrainingConfig, ParameterGolfSubmissionAccountingReceipt,
    ParameterGolfSubmissionError, ParameterGolfSubmissionRuntimeError,
    ParameterGolfSubmissionRuntimeManifest, ParameterGolfSubmissionRuntimeReceipt,
    PARAMETER_GOLF_ACCOUNTING_COMPONENT_BUILD_DEPS, PARAMETER_GOLF_ACCOUNTING_COMPONENT_ENTRYPOINT,
    PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL, PARAMETER_GOLF_ACCOUNTING_COMPONENT_RUNTIME,
    PARAMETER_GOLF_ACCOUNTING_COMPONENT_WRAPPER, PARAMETER_GOLF_NON_RECORD_TRACK_ID,
};

/// Canonical committed report for exported submission run evidence.
pub const PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_submission_run_evidence.json";
/// Canonical committed report for folder-local replay verification.
pub const PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_record_folder_replay_verification.json";
/// Canonical committed report for the final PR bundle generator.
pub const PARAMETER_GOLF_FINAL_PR_BUNDLE_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_final_pr_bundle.json";
/// Canonical committed report for the local-clone dry run.
pub const PARAMETER_GOLF_LOCAL_CLONE_DRY_RUN_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_local_clone_dry_run.json";

const PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_FILE: &str =
    "psionic_parameter_golf_submission_run_evidence.json";
const PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_FILE: &str =
    "psionic_parameter_golf_record_folder_replay_verification.json";
const PARAMETER_GOLF_SUBMISSION_PROMOTION_RECEIPT_FILE: &str =
    "psionic_parameter_golf_submission_promotion_receipt.json";
const PARAMETER_GOLF_PR_CHECKLIST_FILE: &str = "PSIONIC_PARAMETER_GOLF_PR_CHECKLIST.md";
const PARAMETER_GOLF_FINAL_PR_BUNDLE_OUTPUT_FILE: &str = "parameter_golf_final_pr_bundle.json";
const PARAMETER_GOLF_RECORD_FOLDER_COMPATIBILITY_VERIFICATION_FILE: &str =
    "psionic_parameter_golf_record_folder_compatibility_verification.json";

/// Explicit execution posture used to bind one exported submission to the challenge lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionChallengeExecutionPosture {
    /// Stable posture identifier.
    pub posture_id: String,
    /// Honest detail about the local operator posture.
    pub detail: String,
    /// Current visible devices used for review.
    pub devices: Vec<DeviceDescriptor>,
    /// Current capability profile supplied to the distributed receipt lane.
    pub capability_profile: ClusterExecutionCapabilityProfile,
}

impl ParameterGolfSubmissionChallengeExecutionPosture {
    /// Returns the current local review-host posture used for committed evidence.
    #[must_use]
    pub fn local_review_host_defaults() -> Self {
        Self {
            posture_id: String::from("local_review_host_single_rtx4080"),
            detail: String::from(
                "single display-attached RTX 4080 local review host; this posture can bind exported-folder bytes and replay evidence to the challenge lane, but it refuses the true 8xH100 benchmark because the inventory is not challenge-matching",
            ),
            devices: vec![DeviceDescriptor {
                backend: String::from("cuda"),
                device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
                device_name: Some(String::from("NVIDIA GeForce RTX 4080")),
                supported_dtypes: vec![DType::F32, DType::BF16],
                supported_quantization: Vec::new(),
                memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
                unified_memory: Some(false),
                feature_flags: vec![String::from("cuda_architecture_surface")],
                amd_metadata: None,
                nvidia_metadata: Some(NvidiaDeviceMetadata {
                    topology: NvidiaTopologyInfo {
                        architecture: Some(String::from("ada")),
                        compute_capability: Some(String::from("8.9")),
                        pci_bdf: Some(String::from("00000000:01:00.0")),
                        sm_count: Some(76),
                        vram_bytes: Some(16 * 1024 * 1024 * 1024),
                        mig_profile: None,
                    },
                    risk: NvidiaRiskProfile {
                        level: NvidiaRiskLevel::Elevated,
                        display_attached: Some(true),
                        mig_partitioned: false,
                        warnings: vec![String::from(
                            "display-attached NVIDIA devices may show variable latency under local desktop load",
                        )],
                    },
                    recovery: NvidiaRecoveryProfile {
                        supports_gpu_reset: Some(true),
                        expected_actions: vec![
                            NvidiaRecoveryAction::ProcessRestart,
                            NvidiaRecoveryAction::GpuReset,
                            NvidiaRecoveryAction::RebootHost,
                        ],
                    },
                }),
            }],
            capability_profile: ClusterExecutionCapabilityProfile::new("cuda")
                .with_supported_communication_classes(vec![
                    ClusterCommunicationClass::TensorCollectiveMesh,
                ])
                .with_detail(
                    "local review host advertises the same backend family and collective vocabulary, but not the required 8-device challenge inventory",
                ),
        }
    }

    /// Returns the canonical RunPod single-node `8xH100` posture used for real exported-folder evidence.
    #[must_use]
    pub fn runpod_8xh100_defaults() -> Self {
        Self {
            posture_id: String::from("runpod_single_node_8xh100"),
            detail: String::from(
                "single-node RunPod 8xH100 posture; this binds exported-folder evidence to challenge-matching inventory, but the exported entrypoint replay still needs separate distributed timing and memory receipts before it can claim a measured 8xH100 training result",
            ),
            devices: (0..8).map(sample_h100_device).collect(),
            capability_profile: ClusterExecutionCapabilityProfile::new("cuda")
                .with_supported_communication_classes(vec![
                    ClusterCommunicationClass::TensorCollectiveMesh,
                ])
                .with_detail(
                    "single-node RunPod H100 mesh advertises the same backend family and collective vocabulary as the intended challenge lane",
                ),
        }
    }
}

fn sample_h100_device(ordinal: usize) -> DeviceDescriptor {
    DeviceDescriptor {
        backend: String::from("cuda"),
        device: Device::new(
            DeviceKind::Cuda,
            ordinal as u16,
            Some(format!("cuda:{ordinal}")),
        ),
        device_name: Some(String::from("NVIDIA H100 80GB HBM3")),
        supported_dtypes: vec![DType::F32, DType::BF16],
        supported_quantization: Vec::new(),
        memory_capacity_bytes: Some(80 * 1024 * 1024 * 1024),
        unified_memory: Some(false),
        feature_flags: vec![String::from("cuda_architecture_surface")],
        amd_metadata: None,
        nvidia_metadata: Some(NvidiaDeviceMetadata {
            topology: NvidiaTopologyInfo {
                architecture: Some(String::from("hopper")),
                compute_capability: Some(String::from("9.0")),
                pci_bdf: Some(format!("00000000:{:02x}:00.0", ordinal + 1)),
                sm_count: Some(132),
                vram_bytes: Some(80 * 1024 * 1024 * 1024),
                mig_profile: None,
            },
            risk: NvidiaRiskProfile {
                level: NvidiaRiskLevel::Standard,
                display_attached: Some(false),
                mig_partitioned: false,
                warnings: Vec::new(),
            },
            recovery: NvidiaRecoveryProfile {
                supports_gpu_reset: Some(true),
                expected_actions: vec![
                    NvidiaRecoveryAction::ProcessRestart,
                    NvidiaRecoveryAction::GpuReset,
                    NvidiaRecoveryAction::RebootHost,
                ],
            },
        }),
    }
}

/// One observed file inside a generated submission folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionObservedFile {
    /// Relative path from the submission root.
    pub relative_path: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Stable SHA-256 digest over the file bytes.
    pub sha256_digest: String,
}

/// Metric facts lifted from one challenge-facing artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionMetricFacts {
    /// Validation loss.
    pub val_loss: f64,
    /// Validation bits per byte.
    pub val_bpb: f64,
}

/// Artifact-byte facts lifted from one challenge-facing artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionArtifactByteFacts {
    /// Total counted bytes.
    pub total_bytes: u64,
    /// Counted code bytes.
    pub code_bytes: u64,
    /// Counted compressed-model bytes.
    pub model_bytes: u64,
}

/// Verification result for one counted accounting component.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfAccountingComponentVerification {
    /// Stable accounting component identifier.
    pub component_id: String,
    /// Declared size in the accounting receipt.
    pub declared_size_bytes: u64,
    /// Actual size observed in the exported folder under the current contract.
    pub actual_size_bytes: u64,
    /// Whether the declared size matched the actual observed size.
    pub matches_actual_size: bool,
    /// Honest detail about how the component was verified.
    pub detail: String,
}

/// Challenge-facing evidence report over one exported submission folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfSubmissionRunEvidenceReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable package version.
    pub package_version: String,
    /// Stable submission identifier.
    pub submission_id: String,
    /// Submission track identifier.
    pub track_id: String,
    /// Record-folder path used for challenge PRs.
    pub record_folder_relpath: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Explicit challenge execution posture bound to the folder.
    pub challenge_execution_posture: ParameterGolfSubmissionChallengeExecutionPosture,
    /// Top-level entrypoint file.
    pub entrypoint: ParameterGolfSubmissionObservedFile,
    /// Shipped runtime payload file.
    pub runtime_payload: ParameterGolfSubmissionObservedFile,
    /// Shipped runtime manifest file.
    pub runtime_manifest: ParameterGolfSubmissionObservedFile,
    /// Preserved train log file.
    pub train_log: ParameterGolfSubmissionObservedFile,
    /// Counted model artifact file.
    pub model_artifact: ParameterGolfSubmissionObservedFile,
    /// Preserved accounting receipt file.
    pub accounting_receipt: ParameterGolfSubmissionObservedFile,
    /// Preserved benchmark receipt file.
    pub benchmark_receipt: ParameterGolfSubmissionObservedFile,
    /// Preserved run-bundle file.
    pub run_bundle: ParameterGolfSubmissionObservedFile,
    /// Runtime receipt emitted by the exported entrypoint.
    pub runtime_receipt: ParameterGolfSubmissionObservedFile,
    /// Command used for the exported entrypoint dry run.
    pub entrypoint_dry_run_command: Vec<String>,
    /// Exit code returned by the exported entrypoint dry run.
    pub entrypoint_dry_run_exit_code: i32,
    /// Preserved bounded wallclock receipt from the exported folder.
    pub benchmark_wallclock_receipt: ParameterGolfWallclockReceipt,
    /// Preserved bounded memory receipt from the exported folder.
    pub benchmark_memory_receipt: ParameterGolfMemoryReceipt,
    /// Preserved artifact-size receipt from the exported folder.
    pub benchmark_artifact_size_receipt: ParameterGolfArtifactSizeReceipt,
    /// Measured-or-refused challenge receipt bound to the exact exported folder.
    pub distributed_challenge_receipt: ParameterGolfDistributedThroughputReceipt,
    /// Honest claim boundary for this evidence bundle.
    pub claim_boundary: String,
    /// Stable digest over the report payload.
    pub report_digest: String,
}

/// Current replay-verifier verdict for one exported submission folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfRecordFolderReplayVerificationVerdict {
    /// Every checked fact matched the exported folder and preserved receipts.
    Verified,
    /// One or more replay facts drifted.
    Inconsistent,
}

/// Machine-readable folder-local replay verification report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfRecordFolderReplayVerificationReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable submission identifier.
    pub submission_id: String,
    /// Submission track identifier.
    pub track_id: String,
    /// Record-folder path used for challenge PRs.
    pub record_folder_relpath: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Entry-point file used by the verifier.
    pub entrypoint_path: String,
    /// Submission-facing metrics from `submission.json`.
    pub submission_manifest_metrics: ParameterGolfSubmissionMetricFacts,
    /// Final int8+zlib metrics parsed from `train.log`.
    pub train_log_metrics: ParameterGolfSubmissionMetricFacts,
    /// Final int8+zlib metrics from the preserved benchmark receipt.
    pub benchmark_receipt_metrics: ParameterGolfSubmissionMetricFacts,
    /// Final executed metrics from the runtime receipt.
    pub runtime_receipt_metrics: ParameterGolfSubmissionMetricFacts,
    /// Wallclock from `submission.json`.
    pub submission_manifest_wallclock_seconds: f64,
    /// Training wallclock from the preserved benchmark receipt.
    pub benchmark_training_wallclock_seconds: f64,
    /// Bytes from `submission.json`.
    pub submission_manifest_bytes: ParameterGolfSubmissionArtifactByteFacts,
    /// Bytes from the accounting receipt.
    pub accounting_receipt_bytes: ParameterGolfSubmissionArtifactByteFacts,
    /// Actual observed bytes from the exported folder.
    pub actual_shipped_bytes: ParameterGolfSubmissionArtifactByteFacts,
    /// Per-component accounting verification details.
    pub accounting_component_verifications: Vec<ParameterGolfAccountingComponentVerification>,
    /// Whether `submission.json` matched the final `train.log` metrics.
    pub matches_train_log_metrics: bool,
    /// Whether `submission.json` matched the preserved benchmark receipt metrics.
    pub matches_benchmark_receipt_metrics: bool,
    /// Whether `submission.json` matched the executed runtime receipt metrics.
    pub matches_runtime_receipt_metrics: bool,
    /// Whether the wallclock in `submission.json` matched the preserved benchmark receipt.
    pub matches_benchmark_wallclock: bool,
    /// Whether the runtime receipt stayed internally consistent.
    pub runtime_receipt_consistent: bool,
    /// Whether the preserved benchmark artifact-size receipt matched the shipped model bytes.
    pub matches_benchmark_artifact_bytes: bool,
    /// Final replay-verifier verdict.
    pub verdict: ParameterGolfRecordFolderReplayVerificationVerdict,
    /// Honest claim boundary for the verifier.
    pub claim_boundary: String,
    /// Stable digest over the report payload.
    pub report_digest: String,
}

/// Typed compatibility verifier output produced by the folder-compatibility script.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordFolderCompatibilityEntrypointDryRun {
    /// Whether the entrypoint was executed.
    pub executed: bool,
    /// Command used for the dry run.
    pub command: Vec<String>,
    /// Exit code returned by the entrypoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Stable digest over stdout when the entrypoint was executed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_digest: Option<String>,
    /// Short stdout preview.
    pub stdout_preview: String,
    /// Short stderr preview.
    pub stderr_preview: String,
}

/// Full compatibility verifier report returned by the repo-local script.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordFolderCompatibilityVerificationReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Repo-local runner path.
    pub runner: String,
    /// Digest of the canonical compatibility report used by the script.
    pub compatibility_report_digest: String,
    /// Absolute parameter-golf repo root used by the script.
    pub parameter_golf_root: String,
    /// Absolute submission directory used by the script.
    pub submission_dir: String,
    /// Track identifier resolved by the script.
    pub track_id: String,
    /// Records relpath resolved by the script.
    pub records_relpath: String,
    /// Ordered required top-level files checked by the script.
    pub required_top_level_files: Vec<String>,
    /// Ordered extra top-level files present in the folder.
    pub extra_top_level_files: Vec<String>,
    /// Ordered nested files present in the folder.
    pub nested_submission_paths: Vec<String>,
    /// Entry-point dry-run result.
    pub entrypoint_dry_run: ParameterGolfRecordFolderCompatibilityEntrypointDryRun,
    /// Final verifier verdict.
    pub verdict: String,
    /// Stable digest over the verifier payload.
    pub report_digest: String,
}

/// Stable subset of the compatibility verifier report preserved in committed dry runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRecordFolderCompatibilityVerificationSnapshot {
    /// Repo-local runner path.
    pub runner: String,
    /// Digest of the canonical compatibility report used by the script.
    pub compatibility_report_digest: String,
    /// Track identifier resolved by the script.
    pub track_id: String,
    /// Records relpath resolved by the script.
    pub records_relpath: String,
    /// Ordered required top-level files checked by the script.
    pub required_top_level_files: Vec<String>,
    /// Ordered extra top-level files present in the folder.
    pub extra_top_level_files: Vec<String>,
    /// Ordered nested files present in the folder.
    pub nested_submission_paths: Vec<String>,
    /// Exit code returned by the entrypoint dry run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint_dry_run_exit_code: Option<i32>,
    /// Final verifier verdict.
    pub verdict: String,
    /// Stable digest over the verifier payload.
    pub report_digest: String,
}

impl From<ParameterGolfRecordFolderCompatibilityVerificationReport>
    for ParameterGolfRecordFolderCompatibilityVerificationSnapshot
{
    fn from(value: ParameterGolfRecordFolderCompatibilityVerificationReport) -> Self {
        Self {
            runner: value.runner,
            compatibility_report_digest: value.compatibility_report_digest,
            track_id: value.track_id,
            records_relpath: value.records_relpath,
            required_top_level_files: value.required_top_level_files,
            extra_top_level_files: value.extra_top_level_files,
            nested_submission_paths: value.nested_submission_paths,
            entrypoint_dry_run_exit_code: value.entrypoint_dry_run.exit_code,
            verdict: value.verdict,
            report_digest: value.report_digest,
        }
    }
}

/// Final PR bundle generator report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfFinalPrBundleReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable submission identifier.
    pub submission_id: String,
    /// Submission track identifier.
    pub track_id: String,
    /// Record-folder path emitted by the generator.
    pub record_folder_relpath: String,
    /// Relative path to the run-evidence report inside the generated folder.
    pub submission_run_evidence_path: String,
    /// Relative path to the replay-verification report inside the generated folder.
    pub replay_verification_path: String,
    /// Relative path to the promotion receipt inside the generated folder.
    pub promotion_receipt_path: String,
    /// Relative path to the maintainer-facing checklist inside the generated folder.
    pub checklist_path: String,
    /// Ordered generated files inside the submission folder.
    pub generated_submission_files: Vec<ParameterGolfSubmissionObservedFile>,
    /// Ordered review artifacts explicitly called out by the bundle.
    pub required_review_artifacts: Vec<String>,
    /// Honest claim boundary for the generated bundle.
    pub claim_boundary: String,
    /// Stable digest over the report payload.
    pub report_digest: String,
}

/// Final verdict for the local-clone dry-run report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfLocalCloneDryRunVerdict {
    /// The staged bundle verified cleanly and the clone returned to a clean state.
    CleanPass,
    /// The staged bundle failed one or more verifiers.
    VerificationFailed,
    /// The clone did not return to the original clean status.
    DirtyAfter,
}

/// Local-clone dry-run report for the generated submission bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfLocalCloneDryRunReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable submission identifier.
    pub submission_id: String,
    /// Submission track identifier.
    pub track_id: String,
    /// Staged record-folder path inside the local challenge clone.
    pub record_folder_relpath: String,
    /// `git status --short --branch` output before staging the folder.
    pub clone_status_before: String,
    /// `git status --short --branch` output after cleanup.
    pub clone_status_after: String,
    /// Digest of the generated final PR bundle report used by the dry run.
    pub final_pr_bundle_report_digest: String,
    /// Stable compatibility-verifier snapshot from the live local clone.
    pub compatibility_verification: ParameterGolfRecordFolderCompatibilityVerificationSnapshot,
    /// Replay-verifier report from the live local clone.
    pub replay_verification: ParameterGolfRecordFolderReplayVerificationReport,
    /// Final dry-run verdict.
    pub verdict: ParameterGolfLocalCloneDryRunVerdict,
    /// Honest claim boundary for the local-clone dry run.
    pub claim_boundary: String,
    /// Stable digest over the report payload.
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum ParameterGolfSubmissionPrError {
    #[error(transparent)]
    ReferenceTraining(#[from] crate::ParameterGolfReferenceTrainingError),
    #[error(transparent)]
    BenchmarkBundle(#[from] ParameterGolfBenchmarkBundleError),
    #[error(transparent)]
    Submission(#[from] ParameterGolfSubmissionError),
    #[error(transparent)]
    Runtime(#[from] ParameterGolfSubmissionRuntimeError),
    #[error(transparent)]
    Model(#[from] ParameterGolfModelError),
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        artifact_kind: String,
        path: String,
        error: serde_json::Error,
    },
    #[error("missing required submission artifact `{path}`")]
    MissingArtifact { path: String },
    #[error("unknown accounting component `{component_id}` in the current replay verifier")]
    UnknownAccountingComponent { component_id: String },
    #[error("invalid distributed challenge receipt: {message}")]
    InvalidDistributedChallengeReceipt { message: String },
    #[error("invalid parameter golf train.log: {message}")]
    InvalidTrainLog { message: String },
    #[error("command `{command}` failed with exit code {exit_code:?}: {stderr}")]
    CommandFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
    },
    #[error("target path already exists and will not be overwritten: `{path}`")]
    TargetAlreadyExists { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

struct LoadedParameterGolfSubmissionFolder {
    submission_root: PathBuf,
    submission_id: String,
    record_folder_relpath: String,
    submission_manifest: ParameterGolfNonRecordSubmissionManifest,
    accounting_receipt: ParameterGolfSubmissionAccountingReceipt,
    benchmark_receipt: ParameterGolfChallengeBenchmarkReceipt,
    run_bundle: ParameterGolfChallengeRunBundle,
    runtime_manifest: ParameterGolfSubmissionRuntimeManifest,
    runtime_receipt: Option<ParameterGolfSubmissionRuntimeReceipt>,
    train_log: String,
}

/// Returns the canonical absolute path for the committed run-evidence report.
#[must_use]
pub fn parameter_golf_submission_run_evidence_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_REPORT_REF)
}

/// Returns the canonical absolute path for the committed replay-verification report.
#[must_use]
pub fn parameter_golf_record_folder_replay_verification_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_REPORT_REF)
}

/// Returns the canonical absolute path for the committed final PR bundle report.
#[must_use]
pub fn parameter_golf_final_pr_bundle_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_FINAL_PR_BUNDLE_REPORT_REF)
}

/// Returns the canonical absolute path for the committed local-clone dry-run report.
#[must_use]
pub fn parameter_golf_local_clone_dry_run_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_LOCAL_CLONE_DRY_RUN_REPORT_REF)
}

/// Builds the exported-submission evidence report for one submission folder.
pub fn build_parameter_golf_submission_run_evidence_report(
    submission_dir: &Path,
    posture: &ParameterGolfSubmissionChallengeExecutionPosture,
) -> Result<ParameterGolfSubmissionRunEvidenceReport, ParameterGolfSubmissionPrError> {
    build_parameter_golf_submission_run_evidence_report_with_distributed_receipt(
        submission_dir,
        posture,
        None,
    )
}

/// Builds the exported-submission evidence report for one submission folder,
/// optionally binding one pre-measured distributed `8xH100` receipt into the
/// challenge-facing evidence surface.
pub fn build_parameter_golf_submission_run_evidence_report_with_distributed_receipt(
    submission_dir: &Path,
    posture: &ParameterGolfSubmissionChallengeExecutionPosture,
    distributed_receipt_override: Option<&ParameterGolfDistributedThroughputReceipt>,
) -> Result<ParameterGolfSubmissionRunEvidenceReport, ParameterGolfSubmissionPrError> {
    let loaded_before = load_submission_folder(submission_dir)?;
    let exit_code = execute_submission_entrypoint(submission_dir)?;
    let loaded = load_submission_folder(submission_dir)?;
    let runtime_receipt = loaded.runtime_receipt.as_ref().ok_or_else(|| {
        ParameterGolfSubmissionPrError::MissingArtifact {
            path: loaded
                .submission_manifest
                .runtime_receipt_artifact_ref
                .clone(),
        }
    })?;
    let distributed_receipt = build_exported_submission_distributed_receipt(
        &loaded,
        posture,
        distributed_receipt_override,
    )?;
    let claim_boundary = if posture.posture_id == "runpod_single_node_8xh100" {
        if distributed_receipt.disposition == ParameterGolfDistributedLaneDisposition::Measured {
            String::from(
                "This report binds the exact exported submission folder to one real folder-local entrypoint replay on challenge-matching RunPod 8xH100 inventory, the preserved bounded wallclock/memory/artifact receipts, and one measured 8xH100 distributed challenge receipt gathered from real execution evidence.",
            )
        } else {
            String::from(
                "This report binds the exact exported submission folder to one real folder-local entrypoint replay on challenge-matching RunPod 8xH100 inventory, the preserved bounded wallclock/memory/artifact receipts, and one explicit 8xH100 challenge receipt. The exported entrypoint replay still lacks distributed timing and memory measurements, so the challenge receipt remains a measurements-missing refusal rather than true 8xH100 training success.",
            )
        }
    } else {
        String::from(
            "This report binds the exact exported submission folder to one real folder-local entrypoint replay, the preserved bounded wallclock/memory/artifact receipts, and one measured-or-refused 8xH100 challenge receipt. The committed evidence is still a local review-host refusal rather than true 8xH100 success.",
        )
    };
    let mut report = ParameterGolfSubmissionRunEvidenceReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.submission_run_evidence.v1"),
        package_version: loaded.runtime_manifest.package_version.clone(),
        submission_id: loaded.submission_id.clone(),
        track_id: loaded.submission_manifest.track.clone(),
        record_folder_relpath: loaded.record_folder_relpath.clone(),
        run_id: loaded.submission_manifest.run_id.clone(),
        challenge_execution_posture: posture.clone(),
        entrypoint: observed_file(
            submission_dir,
            loaded.submission_manifest.entrypoint.as_str(),
        )?,
        runtime_payload: observed_file(
            submission_dir,
            loaded
                .submission_manifest
                .runtime_payload_artifact_ref
                .as_str(),
        )?,
        runtime_manifest: observed_file(
            submission_dir,
            loaded
                .submission_manifest
                .runtime_manifest_artifact_ref
                .as_str(),
        )?,
        train_log: observed_file(submission_dir, "train.log")?,
        model_artifact: observed_file(
            submission_dir,
            loaded.runtime_manifest.model_artifact_path.as_str(),
        )?,
        accounting_receipt: observed_file(
            submission_dir,
            loaded
                .submission_manifest
                .accounting_receipt_artifact_ref
                .as_str(),
        )?,
        benchmark_receipt: observed_file(
            submission_dir,
            loaded
                .submission_manifest
                .benchmark_receipt_artifact_ref
                .as_str(),
        )?,
        run_bundle: observed_file(
            submission_dir,
            format!(
                "{}/benchmark/run_bundle.json",
                loaded.submission_manifest.run_id
            )
            .as_str(),
        )?,
        runtime_receipt: observed_file(
            submission_dir,
            loaded
                .submission_manifest
                .runtime_receipt_artifact_ref
                .as_str(),
        )?,
        entrypoint_dry_run_command: vec![String::from("python3"), String::from("train_gpt.py")],
        entrypoint_dry_run_exit_code: exit_code,
        benchmark_wallclock_receipt: loaded.benchmark_receipt.wallclock_receipt.clone(),
        benchmark_memory_receipt: loaded.benchmark_receipt.memory_receipt.clone(),
        benchmark_artifact_size_receipt: loaded.benchmark_receipt.artifact_size_receipt.clone(),
        distributed_challenge_receipt: distributed_receipt,
        claim_boundary,
        report_digest: String::new(),
    };
    let _ = runtime_receipt;
    let _ = loaded_before;
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_submission_run_evidence_report|",
        &report,
    );
    Ok(report)
}

/// Writes the canonical exported-submission evidence report by materializing the current bundle in a temporary staging root.
pub fn write_parameter_golf_submission_run_evidence_report(
    output_path: impl AsRef<Path>,
    posture: &ParameterGolfSubmissionChallengeExecutionPosture,
) -> Result<ParameterGolfSubmissionRunEvidenceReport, ParameterGolfSubmissionPrError> {
    with_temporary_generation_dir("submission_run_evidence", |generation_root| {
        let submission_dir = write_canonical_submission_folder(generation_root)?;
        let report = build_parameter_golf_submission_run_evidence_report(&submission_dir, posture)?;
        write_json(output_path.as_ref(), &report)?;
        Ok(report)
    })
}

/// Builds the replay-verification report for one exported submission folder.
pub fn build_parameter_golf_record_folder_replay_verification_report(
    submission_dir: &Path,
) -> Result<ParameterGolfRecordFolderReplayVerificationReport, ParameterGolfSubmissionPrError> {
    let mut loaded = load_submission_folder(submission_dir)?;
    if loaded.runtime_receipt.is_none() {
        let _ = execute_submission_entrypoint(submission_dir)?;
        loaded = load_submission_folder(submission_dir)?;
    }
    let runtime_receipt = loaded.runtime_receipt.as_ref().ok_or_else(|| {
        ParameterGolfSubmissionPrError::MissingArtifact {
            path: loaded
                .submission_manifest
                .runtime_receipt_artifact_ref
                .clone(),
        }
    })?;
    let train_log_metrics = parse_final_train_log_metrics(&loaded.train_log)?;
    let submission_metrics = ParameterGolfSubmissionMetricFacts {
        val_loss: loaded.submission_manifest.val_loss,
        val_bpb: loaded.submission_manifest.val_bpb,
    };
    let benchmark_metrics = ParameterGolfSubmissionMetricFacts {
        val_loss: loaded
            .benchmark_receipt
            .score_report
            .int8_zlib_roundtrip_validation
            .mean_loss,
        val_bpb: loaded
            .benchmark_receipt
            .score_report
            .int8_zlib_roundtrip_validation
            .bits_per_byte,
    };
    let runtime_metrics = ParameterGolfSubmissionMetricFacts {
        val_loss: runtime_receipt.executed_validation.mean_loss,
        val_bpb: runtime_receipt.executed_validation.bits_per_byte,
    };
    let component_verifications = loaded
        .accounting_receipt
        .counted_components
        .iter()
        .map(|component| {
            let actual_size_bytes =
                actual_accounting_component_size(&loaded, component.component_id.as_str())?;
            Ok(ParameterGolfAccountingComponentVerification {
                component_id: component.component_id.clone(),
                declared_size_bytes: component.size_bytes,
                actual_size_bytes,
                matches_actual_size: component.size_bytes == actual_size_bytes,
                detail: component.detail.clone(),
            })
        })
        .collect::<Result<Vec<_>, ParameterGolfSubmissionPrError>>()?;
    let actual_bytes =
        actual_bytes_from_component_verifications(component_verifications.as_slice());
    let accounting_bytes = ParameterGolfSubmissionArtifactByteFacts {
        total_bytes: loaded.accounting_receipt.total_counted_bytes,
        code_bytes: loaded.accounting_receipt.counted_code_bytes,
        model_bytes: loaded.accounting_receipt.compressed_model_bytes,
    };
    let submission_bytes = ParameterGolfSubmissionArtifactByteFacts {
        total_bytes: loaded.submission_manifest.bytes_total,
        code_bytes: loaded.submission_manifest.bytes_code,
        model_bytes: loaded.submission_manifest.bytes_model_int8_zlib,
    };
    let matches_train_log_metrics =
        metrics_match_with_tolerance(&submission_metrics, &train_log_metrics, 1e-8);
    let matches_benchmark_receipt_metrics = metrics_match(&submission_metrics, &benchmark_metrics);
    let matches_runtime_receipt_metrics = metrics_match(&submission_metrics, &runtime_metrics);
    let matches_benchmark_wallclock = wallclock_matches(
        loaded.submission_manifest.wallclock_seconds,
        loaded
            .benchmark_receipt
            .wallclock_receipt
            .training_observed_ms,
    );
    let runtime_receipt_consistent = runtime_receipt.is_consistent();
    let matches_benchmark_artifact_bytes = loaded
        .benchmark_receipt
        .artifact_size_receipt
        .submission_artifact_size_bytes
        == actual_bytes.model_bytes;
    let verdict = if matches_train_log_metrics
        && matches_benchmark_receipt_metrics
        && matches_runtime_receipt_metrics
        && matches_benchmark_wallclock
        && runtime_receipt_consistent
        && matches_benchmark_artifact_bytes
        && component_verifications
            .iter()
            .all(|component| component.matches_actual_size)
        && submission_bytes == actual_bytes
        && accounting_bytes == actual_bytes
    {
        ParameterGolfRecordFolderReplayVerificationVerdict::Verified
    } else {
        ParameterGolfRecordFolderReplayVerificationVerdict::Inconsistent
    };
    let mut report = ParameterGolfRecordFolderReplayVerificationReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.record_folder_replay_verification.v1"),
        submission_id: loaded.submission_id,
        track_id: loaded.submission_manifest.track.clone(),
        record_folder_relpath: loaded.record_folder_relpath,
        run_id: loaded.submission_manifest.run_id.clone(),
        entrypoint_path: loaded.submission_manifest.entrypoint.clone(),
        submission_manifest_metrics: submission_metrics,
        train_log_metrics,
        benchmark_receipt_metrics: benchmark_metrics,
        runtime_receipt_metrics: runtime_metrics,
        submission_manifest_wallclock_seconds: loaded.submission_manifest.wallclock_seconds,
        benchmark_training_wallclock_seconds: loaded.benchmark_receipt.wallclock_receipt.training_observed_ms
            as f64
            / 1_000.0,
        submission_manifest_bytes: submission_bytes,
        accounting_receipt_bytes: accounting_bytes,
        actual_shipped_bytes: actual_bytes,
        accounting_component_verifications: component_verifications,
        matches_train_log_metrics,
        matches_benchmark_receipt_metrics,
        matches_runtime_receipt_metrics,
        matches_benchmark_wallclock,
        runtime_receipt_consistent,
        matches_benchmark_artifact_bytes,
        verdict,
        claim_boundary: String::from(
            "This verifier confirms only the current exported-folder replay path: offline entrypoint execution, final int8+zlib metrics, bounded wallclock facts, and counted-byte facts. It does not by itself turn the bounded non-record replay into a record-track claim.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_record_folder_replay_verification_report|",
        &report,
    );
    Ok(report)
}

/// Writes the canonical replay-verification report by materializing the current bundle in a temporary staging root.
pub fn write_parameter_golf_record_folder_replay_verification_report(
    output_path: impl AsRef<Path>,
) -> Result<ParameterGolfRecordFolderReplayVerificationReport, ParameterGolfSubmissionPrError> {
    with_temporary_generation_dir("record_folder_replay_verification", |generation_root| {
        let submission_dir = write_canonical_submission_folder(generation_root)?;
        let report =
            build_parameter_golf_record_folder_replay_verification_report(&submission_dir)?;
        write_json(output_path.as_ref(), &report)?;
        Ok(report)
    })
}

/// Generates the final PR bundle under one output root and returns the bundle report.
pub fn write_parameter_golf_final_pr_bundle(
    output_root: &Path,
    posture: &ParameterGolfSubmissionChallengeExecutionPosture,
) -> Result<ParameterGolfFinalPrBundleReport, ParameterGolfSubmissionPrError> {
    let submission_dir = write_canonical_submission_folder(output_root)?;
    let loaded = load_submission_folder(&submission_dir)?;
    let evidence_report =
        build_parameter_golf_submission_run_evidence_report(&submission_dir, posture)?;
    write_json(
        &submission_dir.join(PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_FILE),
        &evidence_report,
    )?;
    let replay_report =
        build_parameter_golf_record_folder_replay_verification_report(&submission_dir)?;
    write_json(
        &submission_dir.join(PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_FILE),
        &replay_report,
    )?;
    let promotion_receipt = build_parameter_golf_submission_promotion_receipt(
        ParameterGolfSubmissionPromotionCandidate {
            submission_id: loaded.submission_id.clone(),
            run_id: loaded.submission_manifest.run_id.clone(),
            benchmark_ref: loaded.submission_manifest.benchmark_ref.clone(),
            track_id: loaded.submission_manifest.track.clone(),
            record_track_candidate: loaded.submission_manifest.track
                != PARAMETER_GOLF_NON_RECORD_TRACK_ID,
            val_bpb: loaded.submission_manifest.val_bpb,
            systems_only_waiver_claimed: false,
            systems_only_waiver_supported: false,
            significance_p_value: None,
            significance_evidence_refs: Vec::new(),
            evidence_refs: vec![
                path_inside_record_folder(
                    loaded.record_folder_relpath.as_str(),
                    PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_FILE,
                ),
                path_inside_record_folder(
                    loaded.record_folder_relpath.as_str(),
                    PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_FILE,
                ),
            ],
        },
    );
    write_json(
        &submission_dir.join(PARAMETER_GOLF_SUBMISSION_PROMOTION_RECEIPT_FILE),
        &promotion_receipt,
    )?;
    write_text(
        &submission_dir.join(PARAMETER_GOLF_PR_CHECKLIST_FILE),
        &render_pr_checklist(
            loaded.record_folder_relpath.as_str(),
            loaded.submission_manifest.track.as_str(),
            &promotion_receipt,
        ),
    )?;

    let generated_submission_files = collect_submission_files(&submission_dir)?;
    let required_review_artifacts = vec![
        path_inside_record_folder(loaded.record_folder_relpath.as_str(), "README.md"),
        path_inside_record_folder(loaded.record_folder_relpath.as_str(), "submission.json"),
        path_inside_record_folder(loaded.record_folder_relpath.as_str(), "train.log"),
        path_inside_record_folder(loaded.record_folder_relpath.as_str(), "train_gpt.py"),
        path_inside_record_folder(
            loaded.record_folder_relpath.as_str(),
            PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_FILE,
        ),
        path_inside_record_folder(
            loaded.record_folder_relpath.as_str(),
            PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_FILE,
        ),
        path_inside_record_folder(
            loaded.record_folder_relpath.as_str(),
            PARAMETER_GOLF_SUBMISSION_PROMOTION_RECEIPT_FILE,
        ),
        path_inside_record_folder(
            loaded.record_folder_relpath.as_str(),
            PARAMETER_GOLF_PR_CHECKLIST_FILE,
        ),
    ];
    let mut report = ParameterGolfFinalPrBundleReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.final_pr_bundle.v1"),
        submission_id: loaded.submission_id,
        track_id: loaded.submission_manifest.track,
        record_folder_relpath: loaded.record_folder_relpath.clone(),
        submission_run_evidence_path: path_inside_record_folder(
            loaded.record_folder_relpath.as_str(),
            PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_FILE,
        ),
        replay_verification_path: path_inside_record_folder(
            loaded.record_folder_relpath.as_str(),
            PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_FILE,
        ),
        promotion_receipt_path: path_inside_record_folder(
            loaded.record_folder_relpath.as_str(),
            PARAMETER_GOLF_SUBMISSION_PROMOTION_RECEIPT_FILE,
        ),
        checklist_path: path_inside_record_folder(
            loaded.record_folder_relpath.as_str(),
            PARAMETER_GOLF_PR_CHECKLIST_FILE,
        ),
        generated_submission_files,
        required_review_artifacts,
        claim_boundary: String::from(
            "This bundle is PR-ready for the current non-record lane only. It adds the exact records/... folder plus maintainer-facing evidence and checklist text, but the included promotion receipt still refuses record promotion until true record-track evidence exists.",
        ),
        report_digest: String::new(),
    };
    report.report_digest =
        stable_digest(b"psionic_parameter_golf_final_pr_bundle_report|", &report);
    write_json(
        &output_root.join(PARAMETER_GOLF_FINAL_PR_BUNDLE_OUTPUT_FILE),
        &report,
    )?;
    Ok(report)
}

/// Writes the canonical final PR bundle report to one output path.
pub fn write_parameter_golf_final_pr_bundle_report(
    output_path: impl AsRef<Path>,
    posture: &ParameterGolfSubmissionChallengeExecutionPosture,
) -> Result<ParameterGolfFinalPrBundleReport, ParameterGolfSubmissionPrError> {
    with_temporary_generation_dir("final_pr_bundle", |generation_root| {
        let report = write_parameter_golf_final_pr_bundle(generation_root, posture)?;
        write_json(output_path.as_ref(), &report)?;
        Ok(report)
    })
}

/// Stages a generated bundle into a local `parameter-golf` clone, runs the verifiers, and preserves one dry-run report.
pub fn write_parameter_golf_local_clone_dry_run_report(
    output_path: impl AsRef<Path>,
    parameter_golf_root: &Path,
    posture: &ParameterGolfSubmissionChallengeExecutionPosture,
) -> Result<ParameterGolfLocalCloneDryRunReport, ParameterGolfSubmissionPrError> {
    with_temporary_generation_dir("local_clone_dry_run", |generation_root| {
        let final_bundle = write_parameter_golf_final_pr_bundle(generation_root, posture)?;
        let submission_dir = generation_root.join(&final_bundle.record_folder_relpath);
        let target_dir = parameter_golf_root.join(&final_bundle.record_folder_relpath);
        if target_dir.exists() {
            return Err(ParameterGolfSubmissionPrError::TargetAlreadyExists {
                path: target_dir.display().to_string(),
            });
        }
        let clone_status_before = git_status_short_branch(parameter_golf_root)?;
        copy_dir_all(&submission_dir, &target_dir)?;
        let verification = (|| {
            let compatibility_report =
                run_compatibility_verifier(parameter_golf_root, &target_dir)?;
            let replay_report = run_replay_verifier(&target_dir)?;
            Ok::<_, ParameterGolfSubmissionPrError>((compatibility_report, replay_report))
        })();
        let cleanup_result = fs::remove_dir_all(&target_dir);
        let clone_status_after = git_status_short_branch(parameter_golf_root)?;
        match verification {
            Ok((compatibility_report, replay_report)) => {
                cleanup_result.map_err(|error| ParameterGolfSubmissionPrError::Read {
                    path: target_dir.display().to_string(),
                    error,
                })?;
                let compatibility_snapshot =
                    ParameterGolfRecordFolderCompatibilityVerificationSnapshot::from(
                        compatibility_report,
                    );
                let verdict = if clone_status_before != clone_status_after {
                    ParameterGolfLocalCloneDryRunVerdict::DirtyAfter
                } else if compatibility_snapshot.verdict == "compatible"
                    && replay_report.verdict
                        == ParameterGolfRecordFolderReplayVerificationVerdict::Verified
                {
                    ParameterGolfLocalCloneDryRunVerdict::CleanPass
                } else {
                    ParameterGolfLocalCloneDryRunVerdict::VerificationFailed
                };
                let mut report = ParameterGolfLocalCloneDryRunReport {
                    schema_version: 1,
                    report_id: String::from("parameter_golf.local_clone_dry_run.v1"),
                    submission_id: final_bundle.submission_id,
                    track_id: final_bundle.track_id,
                    record_folder_relpath: final_bundle.record_folder_relpath,
                    clone_status_before,
                    clone_status_after,
                    final_pr_bundle_report_digest: final_bundle.report_digest,
                    compatibility_verification: compatibility_snapshot,
                    replay_verification: replay_report,
                    verdict,
                    claim_boundary: String::from(
                        "This dry run proves that the current generated folder can be staged into a live local parameter-golf clone, reverified there, and cleaned back out without drift. It does not by itself upgrade the bounded non-record lane into record-track readiness.",
                    ),
                    report_digest: String::new(),
                };
                report.report_digest = stable_digest(
                    b"psionic_parameter_golf_local_clone_dry_run_report|",
                    &report,
                );
                write_json(output_path.as_ref(), &report)?;
                Ok(report)
            }
            Err(error) => {
                let _ = cleanup_result;
                Err(error)
            }
        }
    })
}

fn build_exported_submission_distributed_receipt(
    loaded: &LoadedParameterGolfSubmissionFolder,
    posture: &ParameterGolfSubmissionChallengeExecutionPosture,
    distributed_receipt_override: Option<&ParameterGolfDistributedThroughputReceipt>,
) -> Result<ParameterGolfDistributedThroughputReceipt, ParameterGolfSubmissionPrError> {
    if let Some(receipt) = distributed_receipt_override {
        validate_distributed_receipt_matches_posture(receipt, posture)?;
        return Ok(receipt.clone());
    }
    let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
    let hyperparameters = crate::ParameterGolfTrainingHyperparameters::baseline_defaults();
    let thresholds = ParameterGolfDistributedChallengeThresholds::challenge_8xh100();
    let backend_matches = posture.capability_profile.runtime_backend == thresholds.required_backend;
    let communication_matches = posture
        .capability_profile
        .supports_communication_class(ClusterCommunicationClass::TensorCollectiveMesh);
    let device_count_matches = posture.devices.len() == thresholds.required_world_size;
    let all_devices_match_required_model = posture.devices.iter().all(|device| {
        device
            .device_name
            .as_deref()
            .is_some_and(|name| name.contains(thresholds.required_device_name.as_str()))
            && device
                .nvidia_metadata
                .as_ref()
                .is_some_and(|metadata| metadata.topology.mig_profile.is_none())
    });
    let (refusal_kind, rejection_reason) = if !backend_matches {
        (
            ParameterGolfDistributedLaneRefusalKind::CapabilityMismatch,
            format!(
                "cluster capability profile targets backend `{}` instead of required `{}`",
                posture.capability_profile.runtime_backend, thresholds.required_backend
            ),
        )
    } else if !communication_matches {
        (
            ParameterGolfDistributedLaneRefusalKind::CapabilityMismatch,
            String::from(
                "cluster capability profile does not advertise tensor_collective_mesh support required for NCCL-style all-reduce",
            ),
        )
    } else if !device_count_matches {
        (
            ParameterGolfDistributedLaneRefusalKind::DeviceInventoryMismatch,
            format!(
                "expected exactly {} devices for the 8xH100 lane, found {}",
                thresholds.required_world_size,
                posture.devices.len()
            ),
        )
    } else if !all_devices_match_required_model {
        (
            ParameterGolfDistributedLaneRefusalKind::DeviceInventoryMismatch,
            format!(
                "selected inventory is not an exact `{}` CUDA posture with non-MIG devices",
                thresholds.required_device_name
            ),
        )
    } else {
        (
            ParameterGolfDistributedLaneRefusalKind::MeasurementsMissing,
            String::from(
                "challenge-matching 8xH100 inventory is present, but the exported entrypoint replay does not by itself produce distributed timing or memory receipts for the true 8xH100 training path",
            ),
        )
    };
    let topology = ExecutionTopologyPlan::replicated(
        thresholds.required_backend.clone(),
        posture
            .devices
            .iter()
            .map(DeviceDescriptor::inventory_qualifiers)
            .collect(),
    );
    let backend_selection = BackendSelection::refused(
        thresholds.required_backend.clone(),
        posture.devices.first().cloned(),
        vec![String::from("parameter_golf_distributed_train")],
        ServedProductBackendPolicy::same_backend_only(),
        ServedProductFallbackTrigger::RequestedBackendUnavailable,
        rejection_reason.clone(),
    )
    .with_selected_devices(posture.devices.clone())
    .with_execution_topology(Some(topology.clone()));
    let coverage_report = builtin_parameter_golf_cuda_training_capability_report()?;
    let mut boundary_notes = vec![if refusal_kind
        == ParameterGolfDistributedLaneRefusalKind::MeasurementsMissing
    {
        String::from(
            "The current exported-folder evidence is bound to challenge-matching RunPod 8xH100 inventory, but it still lacks distributed timing and memory receipts for the true 8xH100 training path, so the challenge receipt remains an explicit refusal.",
        )
    } else {
        String::from(
            "The current exported-folder evidence is bound to a local review host rather than challenge-matching 8xH100 inventory, so the challenge receipt remains an explicit refusal.",
        )
    }];
    boundary_notes.extend(coverage_report.boundary_notes());
    Ok(ParameterGolfDistributedThroughputReceipt {
        benchmark_ref: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF),
        run_id: format!("{}.exported_submission", loaded.submission_manifest.run_id),
        model_descriptor_digest: model.descriptor().stable_digest(),
        optimizer_plan_digest: stable_digest(
            b"psionic_parameter_golf_submission_exported_hyperparameters|",
            &hyperparameters,
        ),
        thresholds: thresholds.clone(),
        topology: ParameterGolfDistributedTopologyReceipt {
            backend_selection,
            topology_digest: topology.stable_digest(),
            selected_device_names: posture
                .devices
                .iter()
                .map(|device| {
                    device
                        .device_name
                        .clone()
                        .unwrap_or_else(|| String::from("unknown"))
                })
                .collect(),
            all_devices_match_required_model,
        },
        communication: ParameterGolfDistributedCommunicationReceipt {
            communication_class: ClusterCommunicationClass::TensorCollectiveMesh,
            transport: ClusterTransportClass::Loopback,
            mesh_id: String::from("mesh.parameter_golf.exported_submission"),
            axes: Vec::new(),
            stages: Vec::new(),
        },
        training_capability_report_digest: coverage_report.report_digest.clone(),
        challenge_kernel_blockers: coverage_report.challenge_kernel_blockers().to_vec(),
        disposition: ParameterGolfDistributedLaneDisposition::Refused,
        timing: None,
        validation_aggregation: None,
        memory: None,
        refusal: Some(ParameterGolfDistributedLaneRefusal {
            refusal_kind,
            reason: rejection_reason,
            fallback_benchmark_ref: String::from(PARAMETER_GOLF_CHALLENGE_REVIEW_BENCHMARK_REF),
        }),
        boundary_notes,
        claim_boundary: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY),
        receipt_digest: String::new(),
    }
    .with_stable_digest())
}

fn validate_distributed_receipt_matches_posture(
    receipt: &ParameterGolfDistributedThroughputReceipt,
    posture: &ParameterGolfSubmissionChallengeExecutionPosture,
) -> Result<(), ParameterGolfSubmissionPrError> {
    if receipt.benchmark_ref != PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF {
        return Err(ParameterGolfSubmissionPrError::InvalidDistributedChallengeReceipt {
            message: format!(
                "benchmark_ref `{}` does not match the canonical distributed lane `{}`",
                receipt.benchmark_ref, PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF
            ),
        });
    }
    let posture_device_names = posture
        .devices
        .iter()
        .map(|device| {
            device
                .device_name
                .clone()
                .unwrap_or_else(|| String::from("unknown"))
        })
        .collect::<Vec<_>>();
    if receipt.topology.selected_device_names != posture_device_names {
        return Err(ParameterGolfSubmissionPrError::InvalidDistributedChallengeReceipt {
            message: format!(
                "receipt selected_device_names {:?} do not match posture device names {:?}",
                receipt.topology.selected_device_names, posture_device_names
            ),
        });
    }
    if receipt.topology.backend_selection.selection_state == BackendSelectionState::Refused
        && receipt.disposition == ParameterGolfDistributedLaneDisposition::Measured
    {
        return Err(ParameterGolfSubmissionPrError::InvalidDistributedChallengeReceipt {
            message: String::from(
                "a measured distributed receipt must not carry a refused backend selection",
            ),
        });
    }
    Ok(())
}

fn load_submission_folder(
    submission_dir: &Path,
) -> Result<LoadedParameterGolfSubmissionFolder, ParameterGolfSubmissionPrError> {
    let submission_manifest: ParameterGolfNonRecordSubmissionManifest = read_json(
        submission_dir.join("submission.json"),
        "parameter_golf_submission_manifest",
    )?;
    let accounting_receipt: ParameterGolfSubmissionAccountingReceipt = read_json(
        submission_dir.join(&submission_manifest.accounting_receipt_artifact_ref),
        "parameter_golf_submission_accounting_receipt",
    )?;
    let benchmark_receipt: ParameterGolfChallengeBenchmarkReceipt = read_json(
        submission_dir.join(&submission_manifest.benchmark_receipt_artifact_ref),
        "parameter_golf_challenge_benchmark_receipt",
    )?;
    let run_bundle: ParameterGolfChallengeRunBundle = read_json(
        submission_dir.join(format!(
            "{}/benchmark/run_bundle.json",
            submission_manifest.run_id
        )),
        "parameter_golf_run_bundle",
    )?;
    let runtime_manifest: ParameterGolfSubmissionRuntimeManifest = read_json(
        submission_dir.join(&submission_manifest.runtime_manifest_artifact_ref),
        "parameter_golf_submission_runtime_manifest",
    )?;
    let runtime_receipt_path =
        submission_dir.join(&submission_manifest.runtime_receipt_artifact_ref);
    let runtime_receipt = if runtime_receipt_path.is_file() {
        Some(read_json(
            &runtime_receipt_path,
            "parameter_golf_submission_runtime_receipt",
        )?)
    } else {
        None
    };
    let train_log = fs::read_to_string(submission_dir.join("train.log")).map_err(|error| {
        ParameterGolfSubmissionPrError::Read {
            path: submission_dir.join("train.log").display().to_string(),
            error,
        }
    })?;
    let submission_id = submission_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| ParameterGolfSubmissionPrError::MissingArtifact {
            path: submission_dir.display().to_string(),
        })?;
    let record_folder_relpath = format!(
        "{}/{}",
        records_dir_for_track(submission_manifest.track.as_str()),
        submission_id
    );
    Ok(LoadedParameterGolfSubmissionFolder {
        submission_root: submission_dir.to_path_buf(),
        submission_id,
        record_folder_relpath,
        submission_manifest,
        accounting_receipt,
        benchmark_receipt,
        run_bundle,
        runtime_manifest,
        runtime_receipt,
        train_log,
    })
}

fn write_canonical_submission_folder(
    output_root: &Path,
) -> Result<PathBuf, ParameterGolfSubmissionPrError> {
    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let training = ParameterGolfReferenceTrainingConfig::local_reference();
    let benchmark = canonicalize_parameter_golf_local_reference_benchmark_bundle(
        &benchmark_parameter_golf_local_reference(&fixture, &training)?,
    )?;
    let submission = build_parameter_golf_non_record_submission_bundle(
        &benchmark,
        &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
    )?;
    let submission_dir = output_root.join(&submission.package.record_folder_relpath);
    if submission_dir.exists() {
        fs::remove_dir_all(&submission_dir).map_err(|error| {
            ParameterGolfSubmissionPrError::Read {
                path: submission_dir.display().to_string(),
                error,
            }
        })?;
    }
    write_parameter_golf_non_record_submission_bundle(&submission, &submission_dir)?;
    Ok(submission_dir)
}

fn execute_submission_entrypoint(
    submission_dir: &Path,
) -> Result<i32, ParameterGolfSubmissionPrError> {
    let output = Command::new("python3")
        .arg("train_gpt.py")
        .current_dir(submission_dir)
        .output()
        .map_err(|error| ParameterGolfSubmissionPrError::Read {
            path: submission_dir.join("train_gpt.py").display().to_string(),
            error,
        })?;
    if !output.status.success() {
        return Err(ParameterGolfSubmissionPrError::CommandFailed {
            command: String::from("python3 train_gpt.py"),
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output.status.code().unwrap_or(0))
}

fn actual_accounting_component_size(
    loaded: &LoadedParameterGolfSubmissionFolder,
    component_id: &str,
) -> Result<u64, ParameterGolfSubmissionPrError> {
    match component_id {
        PARAMETER_GOLF_ACCOUNTING_COMPONENT_ENTRYPOINT => Ok(observed_file(
            &loaded.submission_root,
            loaded.submission_manifest.entrypoint.as_str(),
        )?
        .size_bytes),
        PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL => Ok(observed_file(
            &loaded.submission_root,
            loaded.runtime_manifest.model_artifact_path.as_str(),
        )?
        .size_bytes),
        PARAMETER_GOLF_ACCOUNTING_COMPONENT_RUNTIME => Ok(observed_file(
            &loaded.submission_root,
            loaded.runtime_manifest.runtime_payload_path.as_str(),
        )?
        .size_bytes
            + observed_file(
                &loaded.submission_root,
                loaded
                    .submission_manifest
                    .real_runtime_payload_artifact_ref
                    .as_str(),
            )?
            .size_bytes),
        PARAMETER_GOLF_ACCOUNTING_COMPONENT_WRAPPER => Ok(0),
        PARAMETER_GOLF_ACCOUNTING_COMPONENT_BUILD_DEPS => Ok(0),
        _ => Err(ParameterGolfSubmissionPrError::UnknownAccountingComponent {
            component_id: String::from(component_id),
        }),
    }
}

fn actual_bytes_from_component_verifications(
    components: &[ParameterGolfAccountingComponentVerification],
) -> ParameterGolfSubmissionArtifactByteFacts {
    let model_bytes = components
        .iter()
        .find(|component| component.component_id == PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL)
        .map_or(0, |component| component.actual_size_bytes);
    let total_bytes = components
        .iter()
        .map(|component| component.actual_size_bytes)
        .sum();
    let code_bytes = components
        .iter()
        .filter(|component| component.component_id != PARAMETER_GOLF_ACCOUNTING_COMPONENT_MODEL)
        .map(|component| component.actual_size_bytes)
        .sum();
    ParameterGolfSubmissionArtifactByteFacts {
        total_bytes,
        code_bytes,
        model_bytes,
    }
}

fn metrics_match(
    left: &ParameterGolfSubmissionMetricFacts,
    right: &ParameterGolfSubmissionMetricFacts,
) -> bool {
    metrics_match_with_tolerance(left, right, 1e-9)
}

fn metrics_match_with_tolerance(
    left: &ParameterGolfSubmissionMetricFacts,
    right: &ParameterGolfSubmissionMetricFacts,
    tolerance: f64,
) -> bool {
    metric_matches_with_tolerance(left.val_loss, right.val_loss, tolerance)
        && metric_matches_with_tolerance(left.val_bpb, right.val_bpb, tolerance)
}

fn metric_matches(actual: f64, expected: f64) -> bool {
    metric_matches_with_tolerance(actual, expected, 1e-9)
}

fn metric_matches_with_tolerance(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance
}

fn wallclock_matches(submission_wallclock_seconds: f64, benchmark_wallclock_ms: u64) -> bool {
    metric_matches(
        submission_wallclock_seconds,
        benchmark_wallclock_ms as f64 / 1_000.0,
    )
}

fn parse_final_train_log_metrics(
    train_log: &str,
) -> Result<ParameterGolfSubmissionMetricFacts, ParameterGolfSubmissionPrError> {
    let line = train_log
        .lines()
        .find(|line| line.starts_with("final_int8_zlib_roundtrip_exact "))
        .ok_or_else(|| ParameterGolfSubmissionPrError::InvalidTrainLog {
            message: String::from(
                "missing `final_int8_zlib_roundtrip_exact` line in exported train.log",
            ),
        })?;
    let mut val_loss = None;
    let mut val_bpb = None;
    for token in line.split_whitespace() {
        if let Some(value) = token.strip_prefix("val_loss:") {
            val_loss = Some(value.parse::<f64>().map_err(|error| {
                ParameterGolfSubmissionPrError::InvalidTrainLog {
                    message: format!("invalid val_loss `{value}`: {error}"),
                }
            })?);
        } else if let Some(value) = token.strip_prefix("val_bpb:") {
            val_bpb = Some(value.parse::<f64>().map_err(|error| {
                ParameterGolfSubmissionPrError::InvalidTrainLog {
                    message: format!("invalid val_bpb `{value}`: {error}"),
                }
            })?);
        }
    }
    Ok(ParameterGolfSubmissionMetricFacts {
        val_loss: val_loss.ok_or_else(|| ParameterGolfSubmissionPrError::InvalidTrainLog {
            message: String::from("missing val_loss in final train.log line"),
        })?,
        val_bpb: val_bpb.ok_or_else(|| ParameterGolfSubmissionPrError::InvalidTrainLog {
            message: String::from("missing val_bpb in final train.log line"),
        })?,
    })
}

fn run_compatibility_verifier(
    parameter_golf_root: &Path,
    submission_dir: &Path,
) -> Result<ParameterGolfRecordFolderCompatibilityVerificationReport, ParameterGolfSubmissionPrError>
{
    let report_path =
        submission_dir.join(PARAMETER_GOLF_RECORD_FOLDER_COMPATIBILITY_VERIFICATION_FILE);
    let output = Command::new("bash")
        .arg(repo_root().join("scripts/check-parameter-golf-record-folder-compatibility.sh"))
        .arg("--parameter-golf-root")
        .arg(parameter_golf_root)
        .arg("--submission-dir")
        .arg(submission_dir)
        .arg("--report")
        .arg(&report_path)
        .output()
        .map_err(|error| ParameterGolfSubmissionPrError::Read {
            path: report_path.display().to_string(),
            error,
        })?;
    if !output.status.success() {
        return Err(ParameterGolfSubmissionPrError::CommandFailed {
            command: String::from("scripts/check-parameter-golf-record-folder-compatibility.sh"),
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    read_json(
        &report_path,
        "parameter_golf_record_folder_compatibility_verification_report",
    )
}

fn run_replay_verifier(
    submission_dir: &Path,
) -> Result<ParameterGolfRecordFolderReplayVerificationReport, ParameterGolfSubmissionPrError> {
    let report_path = submission_dir.join(PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_FILE);
    let output = Command::new("bash")
        .arg(repo_root().join("scripts/check-parameter-golf-record-folder-replay.sh"))
        .arg("--submission-dir")
        .arg(submission_dir)
        .arg("--report")
        .arg(&report_path)
        .output()
        .map_err(|error| ParameterGolfSubmissionPrError::Read {
            path: report_path.display().to_string(),
            error,
        })?;
    if !output.status.success() {
        return Err(ParameterGolfSubmissionPrError::CommandFailed {
            command: String::from("scripts/check-parameter-golf-record-folder-replay.sh"),
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    read_json(
        &report_path,
        "parameter_golf_record_folder_replay_verification_report",
    )
}

fn collect_submission_files(
    submission_dir: &Path,
) -> Result<Vec<ParameterGolfSubmissionObservedFile>, ParameterGolfSubmissionPrError> {
    let mut relative_paths = Vec::new();
    collect_relative_file_paths(submission_dir, submission_dir, &mut relative_paths)?;
    relative_paths.sort();
    relative_paths
        .iter()
        .map(|relative_path| observed_file(submission_dir, relative_path.as_str()))
        .collect()
}

fn collect_relative_file_paths(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
) -> Result<(), ParameterGolfSubmissionPrError> {
    for entry in fs::read_dir(current).map_err(|error| ParameterGolfSubmissionPrError::Read {
        path: current.display().to_string(),
        error,
    })? {
        let entry = entry.map_err(|error| ParameterGolfSubmissionPrError::Read {
            path: current.display().to_string(),
            error,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_relative_file_paths(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("file should remain under submission root")
                .to_string_lossy()
                .replace('\\', "/");
            files.push(relative);
        }
    }
    Ok(())
}

fn observed_file(
    root: &Path,
    relative_path: &str,
) -> Result<ParameterGolfSubmissionObservedFile, ParameterGolfSubmissionPrError> {
    let path = root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| ParameterGolfSubmissionPrError::Read {
        path: path.display().to_string(),
        error,
    })?;
    Ok(ParameterGolfSubmissionObservedFile {
        relative_path: String::from(relative_path),
        size_bytes: bytes.len() as u64,
        sha256_digest: sha256_bytes(&bytes),
    })
}

fn path_inside_record_folder(record_folder_relpath: &str, file_name: &str) -> String {
    format!("{record_folder_relpath}/{file_name}")
}

fn records_dir_for_track(track_id: &str) -> &'static str {
    if track_id == PARAMETER_GOLF_NON_RECORD_TRACK_ID {
        "records/track_non_record_16mb"
    } else {
        "records/track_10min_16mb"
    }
}

fn render_pr_checklist(
    record_folder_relpath: &str,
    track_id: &str,
    promotion_receipt: &ParameterGolfSubmissionPromotionReceipt,
) -> String {
    let mut checklist = String::new();
    checklist.push_str("# Psionic Parameter Golf PR Checklist\n\n");
    checklist.push_str("- Confirm the PR adds exactly one new folder under `");
    checklist.push_str(record_folder_relpath);
    checklist.push_str("`.\n");
    checklist.push_str("- Confirm the folder remains self-contained with `README.md`, `submission.json`, `train.log`, `train_gpt.py`, and the shipped runtime payload.\n");
    checklist.push_str("- Review `psionic_parameter_golf_submission_run_evidence.json` for exact entrypoint, runtime, model, and receipt digests.\n");
    checklist.push_str("- Review `psionic_parameter_golf_record_folder_replay_verification.json` for metric, wallclock, and counted-byte replay facts.\n");
    checklist.push_str("- Review `psionic_parameter_golf_submission_promotion_receipt.json` before making any record or waiver claim.\n");
    checklist.push_str("- Re-run `scripts/check-parameter-golf-record-folder-compatibility.sh` and `scripts/check-parameter-golf-record-folder-replay.sh` against the staged folder in the live challenge repo.\n");
    checklist.push_str("- Preserve explicit claim language: this bundle targets track `");
    checklist.push_str(track_id);
    checklist.push_str("`, and the current promotion receipt disposition is `");
    checklist.push_str(match promotion_receipt.disposition {
        ParameterGolfSubmissionPromotionDisposition::Promotable => "promotable",
        ParameterGolfSubmissionPromotionDisposition::Refused => "refused",
    });
    checklist.push_str("`.\n");
    checklist
}

fn with_temporary_generation_dir<T>(
    label: &str,
    f: impl FnOnce(&Path) -> Result<T, ParameterGolfSubmissionPrError>,
) -> Result<T, ParameterGolfSubmissionPrError> {
    let path = std::env::temp_dir().join(format!(
        "psionic_parameter_golf_{}_{}",
        label,
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path).map_err(|error| ParameterGolfSubmissionPrError::Read {
            path: path.display().to_string(),
            error,
        })?;
    }
    fs::create_dir_all(&path).map_err(|error| ParameterGolfSubmissionPrError::CreateDir {
        path: path.display().to_string(),
        error,
    })?;
    let result = f(&path);
    let cleanup = fs::remove_dir_all(&path).map_err(|error| ParameterGolfSubmissionPrError::Read {
        path: path.display().to_string(),
        error,
    });
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn git_status_short_branch(root: &Path) -> Result<String, ParameterGolfSubmissionPrError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--short")
        .arg("--branch")
        .output()
        .map_err(|error| ParameterGolfSubmissionPrError::Read {
            path: root.display().to_string(),
            error,
        })?;
    if !output.status.success() {
        return Err(ParameterGolfSubmissionPrError::CommandFailed {
            command: format!("git -C {} status --short --branch", root.display()),
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), ParameterGolfSubmissionPrError> {
    fs::create_dir_all(dst).map_err(|error| ParameterGolfSubmissionPrError::CreateDir {
        path: dst.display().to_string(),
        error,
    })?;
    for entry in fs::read_dir(src).map_err(|error| ParameterGolfSubmissionPrError::Read {
        path: src.display().to_string(),
        error,
    })? {
        let entry = entry.map_err(|error| ParameterGolfSubmissionPrError::Read {
            path: src.display().to_string(),
            error,
        })?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|error| ParameterGolfSubmissionPrError::Write {
                path: target.display().to_string(),
                error,
            })?;
        }
    }
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ParameterGolfSubmissionPrError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| ParameterGolfSubmissionPrError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{json}\n")).map_err(|error| ParameterGolfSubmissionPrError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn write_text(path: &Path, contents: &str) -> Result<(), ParameterGolfSubmissionPrError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| ParameterGolfSubmissionPrError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    fs::write(path, contents).map_err(|error| ParameterGolfSubmissionPrError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: impl AsRef<Path>,
    artifact_kind: &'static str,
) -> Result<T, ParameterGolfSubmissionPrError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| ParameterGolfSubmissionPrError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| ParameterGolfSubmissionPrError::Deserialize {
        artifact_kind: String::from(artifact_kind),
        path: path.display().to_string(),
        error,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, ParameterGolfSubmissionPrError> {
    read_json(repo_root().join(relative_path), "parameter_golf_repo_json")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs, path::PathBuf, process::Command};

    use psionic_models::ParameterGolfReferenceModel;

    use crate::{
        benchmark_parameter_golf_distributed_8xh100, ParameterGolfDistributed8xH100Config,
        ParameterGolfDistributedStepObservation, ParameterGolfTrainingHyperparameters,
    };

    use super::{
        build_parameter_golf_record_folder_replay_verification_report,
        build_parameter_golf_submission_run_evidence_report,
        build_parameter_golf_submission_run_evidence_report_with_distributed_receipt,
        parameter_golf_final_pr_bundle_report_path,
        parameter_golf_record_folder_replay_verification_report_path,
        parameter_golf_submission_run_evidence_report_path, read_repo_json, repo_root,
        write_canonical_submission_folder, write_parameter_golf_final_pr_bundle,
        write_parameter_golf_final_pr_bundle_report,
        write_parameter_golf_local_clone_dry_run_report,
        write_parameter_golf_record_folder_replay_verification_report,
        write_parameter_golf_submission_run_evidence_report,
        ParameterGolfDistributedLaneDisposition, ParameterGolfDistributedLaneRefusalKind,
        ParameterGolfFinalPrBundleReport, ParameterGolfLocalCloneDryRunReport,
        ParameterGolfLocalCloneDryRunVerdict, ParameterGolfRecordFolderReplayVerificationReport,
        ParameterGolfRecordFolderReplayVerificationVerdict,
        ParameterGolfSubmissionChallengeExecutionPosture, ParameterGolfSubmissionRunEvidenceReport,
        PARAMETER_GOLF_FINAL_PR_BUNDLE_OUTPUT_FILE, PARAMETER_GOLF_FINAL_PR_BUNDLE_REPORT_REF,
        PARAMETER_GOLF_LOCAL_CLONE_DRY_RUN_REPORT_REF,
        PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_REPORT_REF,
        PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_REPORT_REF,
    };

    /// Returns true when the committed replay payload can execute here; otherwise
    /// prints an explicit skip reason naming the architecture mismatch.
    fn committed_runtime_payload_executes_on_host(test_name: &str) -> bool {
        if crate::parameter_golf_submission_runtime_payload_matches_host() {
            return true;
        }
        eprintln!(
            "skipping {test_name}: committed parameter_golf_submission_runtime payload \
             targets x86_64-unknown-linux-gnu and cannot execute on this {}-{} host",
            std::env::consts::ARCH,
            std::env::consts::OS,
        );
        false
    }

    fn measured_runpod_distributed_receipt(
        posture: &ParameterGolfSubmissionChallengeExecutionPosture,
    ) -> Result<psionic_eval::ParameterGolfDistributedThroughputReceipt, Box<dyn Error>> {
        let mut config = ParameterGolfDistributed8xH100Config::challenge_defaults();
        config.run_id = String::from("parameter-golf-runpod-8xh100-measured-test");
        config.step_observations = vec![
            ParameterGolfDistributedStepObservation::new(1, 0, 35, 524_288),
            ParameterGolfDistributedStepObservation::new(2, 35, 72, 524_288),
        ];
        config.validation_observed_ms = 180_000;
        config.export_observed_ms = 15_000;
        let model = ParameterGolfReferenceModel::baseline_fixture(Default::default())?;
        Ok(benchmark_parameter_golf_distributed_8xh100(
            model.descriptor(),
            &ParameterGolfTrainingHyperparameters::baseline_defaults(),
            posture.devices.as_slice(),
            &posture.capability_profile,
            &config,
        )?)
    }

    #[test]
    fn parameter_golf_submission_run_evidence_report_matches_committed_truth(
    ) -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "parameter_golf_submission_run_evidence_report_matches_committed_truth",
        ) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let submission_dir = write_canonical_submission_folder(temp_dir.path())?;
        let generated = build_parameter_golf_submission_run_evidence_report(
            &submission_dir,
            &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
        )?;
        let committed: ParameterGolfSubmissionRunEvidenceReport =
            read_repo_json(PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn runpod_8xh100_posture_keeps_exported_submission_evidence_explicitly_blocked(
    ) -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "runpod_8xh100_posture_keeps_exported_submission_evidence_explicitly_blocked",
        ) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let submission_dir = write_canonical_submission_folder(temp_dir.path())?;
        let generated = build_parameter_golf_submission_run_evidence_report(
            &submission_dir,
            &ParameterGolfSubmissionChallengeExecutionPosture::runpod_8xh100_defaults(),
        )?;
        assert_eq!(
            generated.challenge_execution_posture.posture_id,
            "runpod_single_node_8xh100"
        );
        assert_eq!(
            generated.distributed_challenge_receipt.disposition,
            ParameterGolfDistributedLaneDisposition::Refused
        );
        assert_eq!(
            generated
                .distributed_challenge_receipt
                .refusal
                .as_ref()
                .map(|refusal| refusal.refusal_kind),
            Some(ParameterGolfDistributedLaneRefusalKind::MeasurementsMissing)
        );
        assert!(
            generated
                .distributed_challenge_receipt
                .topology
                .all_devices_match_required_model
        );
        assert!(generated
            .claim_boundary
            .contains("challenge-matching RunPod 8xH100 inventory"));
        Ok(())
    }

    #[test]
    fn runpod_8xh100_posture_accepts_one_measured_distributed_receipt(
    ) -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "runpod_8xh100_posture_accepts_one_measured_distributed_receipt",
        ) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let submission_dir = write_canonical_submission_folder(temp_dir.path())?;
        let posture = ParameterGolfSubmissionChallengeExecutionPosture::runpod_8xh100_defaults();
        let distributed_receipt = measured_runpod_distributed_receipt(&posture)?;
        let generated = build_parameter_golf_submission_run_evidence_report_with_distributed_receipt(
            &submission_dir,
            &posture,
            Some(&distributed_receipt),
        )?;
        assert_eq!(
            generated.distributed_challenge_receipt.disposition,
            ParameterGolfDistributedLaneDisposition::Measured
        );
        assert!(generated.distributed_challenge_receipt.refusal.is_none());
        assert!(generated
            .claim_boundary
            .contains("measured 8xH100 distributed challenge receipt"));
        Ok(())
    }

    #[test]
    fn parameter_golf_record_folder_replay_verification_report_matches_committed_truth(
    ) -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "parameter_golf_record_folder_replay_verification_report_matches_committed_truth",
        ) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let submission_dir = write_canonical_submission_folder(temp_dir.path())?;
        let generated =
            build_parameter_golf_record_folder_replay_verification_report(&submission_dir)?;
        let committed: ParameterGolfRecordFolderReplayVerificationReport =
            read_repo_json(PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_parameter_golf_final_pr_bundle_generates_bundle_files() -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "write_parameter_golf_final_pr_bundle_generates_bundle_files",
        ) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let report = write_parameter_golf_final_pr_bundle(
            temp_dir.path(),
            &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
        )?;
        assert!(temp_dir
            .path()
            .join(PARAMETER_GOLF_FINAL_PR_BUNDLE_OUTPUT_FILE)
            .is_file());
        assert!(temp_dir
            .path()
            .join(&report.record_folder_relpath)
            .join("PSIONIC_PARAMETER_GOLF_PR_CHECKLIST.md")
            .is_file());
        Ok(())
    }

    #[test]
    fn write_parameter_golf_local_clone_dry_run_report_replays_into_mock_clone(
    ) -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "write_parameter_golf_local_clone_dry_run_report_replays_into_mock_clone",
        ) {
            return Ok(());
        }
        let clone_root = build_mock_parameter_golf_clone()?;
        let output_path = clone_root.join("dry_run_report.json");
        let report = write_parameter_golf_local_clone_dry_run_report(
            &output_path,
            &clone_root,
            &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
        )?;
        assert_eq!(
            report.verdict,
            ParameterGolfLocalCloneDryRunVerdict::CleanPass
        );
        assert_eq!(report.compatibility_verification.verdict, "compatible");
        assert_eq!(
            report.replay_verification.verdict,
            ParameterGolfRecordFolderReplayVerificationVerdict::Verified
        );
        Ok(())
    }

    #[test]
    fn committed_local_clone_dry_run_report_stays_green() -> Result<(), Box<dyn Error>> {
        let report: ParameterGolfLocalCloneDryRunReport =
            read_repo_json(PARAMETER_GOLF_LOCAL_CLONE_DRY_RUN_REPORT_REF)?;
        assert_eq!(
            report.verdict,
            ParameterGolfLocalCloneDryRunVerdict::CleanPass
        );
        assert_eq!(report.compatibility_verification.verdict, "compatible");
        assert_eq!(
            report.replay_verification.verdict,
            ParameterGolfRecordFolderReplayVerificationVerdict::Verified
        );
        Ok(())
    }

    #[test]
    fn write_parameter_golf_submission_run_evidence_report_persists_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "write_parameter_golf_submission_run_evidence_report_persists_current_truth",
        ) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let output_path = temp_dir
            .path()
            .join("parameter_golf_submission_run_evidence.json");
        let written = write_parameter_golf_submission_run_evidence_report(
            &output_path,
            &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
        )?;
        let persisted: ParameterGolfSubmissionRunEvidenceReport =
            serde_json::from_slice(&fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            parameter_golf_submission_run_evidence_report_path(),
            repo_root().join(PARAMETER_GOLF_SUBMISSION_RUN_EVIDENCE_REPORT_REF)
        );
        Ok(())
    }

    #[test]
    fn write_parameter_golf_record_folder_replay_verification_report_persists_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "write_parameter_golf_record_folder_replay_verification_report_persists_current_truth",
        ) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let output_path = temp_dir
            .path()
            .join("parameter_golf_record_folder_replay_verification.json");
        let written = write_parameter_golf_record_folder_replay_verification_report(&output_path)?;
        let persisted: ParameterGolfRecordFolderReplayVerificationReport =
            serde_json::from_slice(&fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            parameter_golf_record_folder_replay_verification_report_path(),
            repo_root().join(PARAMETER_GOLF_RECORD_FOLDER_REPLAY_VERIFICATION_REPORT_REF)
        );
        Ok(())
    }

    #[test]
    fn write_parameter_golf_final_pr_bundle_report_persists_current_truth(
    ) -> Result<(), Box<dyn Error>> {
        if !committed_runtime_payload_executes_on_host(
            "write_parameter_golf_final_pr_bundle_report_persists_current_truth",
        ) {
            return Ok(());
        }
        let temp_dir = tempfile::tempdir()?;
        let output_path = temp_dir.path().join("parameter_golf_final_pr_bundle.json");
        let written = write_parameter_golf_final_pr_bundle_report(
            &output_path,
            &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
        )?;
        let persisted: ParameterGolfFinalPrBundleReport =
            serde_json::from_slice(&fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            parameter_golf_final_pr_bundle_report_path(),
            repo_root().join(PARAMETER_GOLF_FINAL_PR_BUNDLE_REPORT_REF)
        );
        Ok(())
    }

    fn build_mock_parameter_golf_clone() -> Result<PathBuf, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = temp_dir.keep();
        fs::write(root.join("README.md"), "# mock parameter golf\n")?;
        fs::create_dir_all(root.join("records/track_10min_16mb/2026-03-17_NaiveBaseline"))?;
        fs::write(
            root.join("records/track_10min_16mb/2026-03-17_NaiveBaseline/README.md"),
            "record example\n",
        )?;
        fs::create_dir_all(root.join(
            "records/track_non_record_16mb/2026-03-18_Quasi10Bfrom50B_SP1024_9x512_KV4_4h_pgut3",
        ))?;
        fs::write(
            root.join("records/track_non_record_16mb/2026-03-18_Quasi10Bfrom50B_SP1024_9x512_KV4_4h_pgut3/README.md"),
            "non-record example\n",
        )?;
        Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("init")
            .output()?;
        Ok(root)
    }
}
