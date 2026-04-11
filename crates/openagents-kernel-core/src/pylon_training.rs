use std::collections::{BTreeMap, BTreeSet};

use bitcoin::hashes::{Hash, sha256};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{compute::ComputeAdapterDatasetSlice, ids::sha256_prefixed_bytes};

pub const PYLON_TRAINING_RUN_MANIFEST_V1: &str = "openagents.pylon_training_run_manifest.v1";
pub const PYLON_TRAINING_EXECUTION_BACKEND_PSIONIC_TRAIN: &str = "psionic_train";
pub const PYLON_TRAINING_GCS_CREDENTIAL_SOURCE: &str = "google_application_default_credentials";
pub const PYLON_TRAINING_TRN_ASSIGNMENT_RECEIPT_KIND: u32 = 39_511;
pub const PYLON_TRAINING_MVP_SETTLEMENT_TRIGGER: &str = "accepted_sealed_window";
pub const PYLON_TRAINING_CUDA_ENVIRONMENT_REF: &str = "env.openagents.cuda.train";
pub const PYLON_TRAINING_APPLE_ENVIRONMENT_REF: &str =
    "psionic.environment.psion_apple_windowed_training.metal_mlx.operator@v1";

pub const PYLON_TRAINING_HEARTBEAT_INTERVAL_MS: u64 = 15_000;
pub const PYLON_TRAINING_HEARTBEAT_EXPIRY_MS: u64 = 60_000;
pub const PYLON_TRAINING_LEASE_DURATION_MS: u64 = 600_000;
pub const PYLON_TRAINING_LEASE_RENEWAL_THRESHOLD_MS: u64 = 180_000;
pub const PYLON_TRAINING_WINDOW_MAX_DURATION_MS: u64 = 1_800_000;
pub const PYLON_TRAINING_SEAL_GRACE_PERIOD_MS: u64 = 120_000;
pub const PYLON_TRAINING_VALIDATOR_TIMEOUT_MS: u64 = 900_000;
pub const PYLON_TRAINING_UPLOAD_TIMEOUT_MS: u64 = 1_200_000;
pub const PYLON_TRAINING_RETRY_SCHEDULE_MS: [u64; 5] = [5_000, 15_000, 30_000, 60_000, 120_000];
pub const PYLON_TRAINING_RETRY_CAP_MS: u64 = 300_000;

pub const PYLON_TRAINING_OBSERVABILITY_FIELDS: [&str; 8] = [
    "network_id",
    "run_id",
    "window_id",
    "assignment_id",
    "challenge_id",
    "node_pubkey",
    "membership_revision",
    "manifest_digest",
];

type ContractResult<T> = Result<T, String>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingAcceptedUnit {
    SealedWindow,
}

pub const PYLON_TRAINING_MVP_ACCEPTED_UNIT: PylonTrainingAcceptedUnit =
    PylonTrainingAcceptedUnit::SealedWindow;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingManifestRole {
    Worker,
    Validator,
    RecoverySource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingExecutionBackend {
    PsionicTrain,
}

impl PylonTrainingExecutionBackend {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PsionicTrain => PYLON_TRAINING_EXECUTION_BACKEND_PSIONIC_TRAIN,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingTopologyBackendFamily {
    Cuda,
    Mlx,
    Metal,
    Mixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingCollectiveKind {
    DataParallel,
    TensorParallel,
    PipelineParallel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingElasticBoundary {
    Window,
    MidWindow,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingTopology {
    pub backend_family: PylonTrainingTopologyBackendFamily,
    pub world_size: u32,
    pub rank: u32,
    #[serde(default)]
    pub local_device_ids: Vec<u32>,
    pub collective_kind: PylonTrainingCollectiveKind,
    pub elastic_boundary: PylonTrainingElasticBoundary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingCheckpointBinding {
    pub checkpoint_family: String,
    pub checkpoint_ref: String,
    pub manifest_digest: String,
    pub latest_pointer_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingArtifacts {
    pub bucket_uri: String,
    pub run_prefix: String,
    pub window_prefix: String,
    pub local_run_root: String,
    pub credential_source: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingTrn {
    pub network_coordinate: String,
    pub window_coordinate: String,
    #[serde(default)]
    pub relay_urls: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingDatasetAssignment {
    pub dataset_id: String,
    pub slice_id: String,
    pub slice_digest: String,
    pub assignment_seed: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingValidatorAssignment {
    pub challenge_id: String,
    pub challenge_kind: String,
    #[serde(default)]
    pub target_assignment_ids: Vec<String>,
    #[serde(default)]
    pub expected_manifest_digests: Vec<String>,
    pub retry_attempt: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingResumeFrom {
    pub checkpoint_ref: String,
    pub latest_pointer_ref: String,
    pub manifest_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingRunManifestV1 {
    pub schema_version: String,
    pub manifest_id: String,
    pub manifest_digest: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub network_id: String,
    pub run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub lease_id: String,
    pub lease_sequence: u64,
    pub membership_revision: String,
    pub role: PylonTrainingManifestRole,
    pub node_pubkey: String,
    pub coordinator_pubkey: String,
    pub authority_base_url: String,
    pub training_policy_ref: String,
    pub validator_policy_ref: String,
    pub environment_ref: String,
    pub environment_version: String,
    pub execution_backend: PylonTrainingExecutionBackend,
    pub topology: PylonTrainingTopology,
    pub checkpoint: PylonTrainingCheckpointBinding,
    pub artifacts: PylonTrainingArtifacts,
    pub trn: PylonTrainingTrn,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<PylonTrainingDatasetAssignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator: Option<PylonTrainingValidatorAssignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_from: Option<PylonTrainingResumeFrom>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingRunManifestCommon {
    pub manifest_id: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub network_id: String,
    pub run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub lease_id: String,
    pub lease_sequence: u64,
    pub membership_revision: String,
    pub node_pubkey: String,
    pub coordinator_pubkey: String,
    pub authority_base_url: String,
    pub training_policy_ref: String,
    pub validator_policy_ref: String,
    pub environment_ref: String,
    pub environment_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingRunManifestBuilder {
    manifest: PylonTrainingRunManifestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingAssignmentPublishedReceipt {
    pub kind: u32,
    pub assignment_id: String,
    pub lease_id: String,
    pub manifest_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingArtifactLayout {
    pub bucket_uri: String,
    pub network_id: String,
    pub run_id: String,
    pub window_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingArtifactLocatorTags {
    pub x: String,
    pub manifest: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PylonTrainingCredentialResolutionMethod {
    EnvironmentVariable,
    InstanceMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingCredentialResolution {
    pub persistent_credential_source: String,
    pub resolution_method: PylonTrainingCredentialResolutionMethod,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PylonTrainingArtifactBundleKind {
    RunManifest,
    LatestCheckpointPointer,
    CheckpointManifest { optimizer_step: u64 },
    Contribution { assignment_id: String },
    ValidatorVerdict { challenge_id: String },
    SealedWindow,
    ScoreSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingArtifactBundleState {
    LocalOnly,
    Staged,
    Uploaded,
    Verified,
    Published,
    Accepted,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PylonTrainingArtifactBundleProgress {
    pub required_objects: BTreeSet<String>,
    pub uploaded_objects: BTreeSet<String>,
    pub digest_matched_objects: BTreeSet<String>,
    pub scheduler_verified: bool,
    pub publication_succeeded: bool,
    pub accepted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingContributionSampleCandidate {
    pub assignment_id: String,
    pub aggregation_weight_bps: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingContributionVerdict {
    Accepted,
    Quarantined,
    Rejected,
    ReplayRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PylonTrainingAggregateResolution {
    Accept,
    Terminal(PylonTrainingContributionVerdict),
    Escalate,
    Held,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PylonTrainingAuthorityOwner {
    Pylon,
    Nexus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PylonTrainingFailureScenario {
    LeaseExpiryDuringUpload,
    PartialCheckpointPublish,
    WorkerDrainDuringSeal,
    CrashAfterLocalCheckpointBeforeUpload,
    TrnPublicationFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingFailureResolution {
    pub owner: PylonTrainingAuthorityOwner,
    pub state: &'static str,
    pub detail: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingReputationNamespace {
    Contributor,
    Validator,
    Build,
    Checkpoint,
}

impl PylonTrainingReputationNamespace {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Contributor => "trn/contributor",
            Self::Validator => "trn/validator",
            Self::Build => "trn/build",
            Self::Checkpoint => "trn/checkpoint",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "trn/contributor" => Some(Self::Contributor),
            "trn/validator" => Some(Self::Validator),
            "trn/build" => Some(Self::Build),
            "trn/checkpoint" => Some(Self::Checkpoint),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingReputationLabel {
    Good,
    Poor,
    Quarantined,
    Fraud,
    Inconsistent,
    Admitted,
    Stale,
    Revoked,
    Warning,
}

impl PylonTrainingReputationLabel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Poor => "poor",
            Self::Quarantined => "quarantined",
            Self::Fraud => "fraud",
            Self::Inconsistent => "inconsistent",
            Self::Admitted => "admitted",
            Self::Stale => "stale",
            Self::Revoked => "revoked",
            Self::Warning => "warning",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "good" => Some(Self::Good),
            "poor" => Some(Self::Poor),
            "quarantined" => Some(Self::Quarantined),
            "fraud" => Some(Self::Fraud),
            "inconsistent" => Some(Self::Inconsistent),
            "admitted" => Some(Self::Admitted),
            "stale" => Some(Self::Stale),
            "revoked" => Some(Self::Revoked),
            "warning" => Some(Self::Warning),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingReputationRecord {
    pub authority_owner: PylonTrainingAuthorityOwner,
    pub namespace: PylonTrainingReputationNamespace,
    pub label: PylonTrainingReputationLabel,
    pub subject_pubkey: Option<String>,
    pub event_ref: Option<String>,
    pub address_ref: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PylonTrainingSchedulerEffect {
    HardGate,
    SoftPositive,
    SoftNegative,
    Ignored,
}

impl PylonTrainingSchedulerEffect {
    pub const fn label(self) -> &'static str {
        match self {
            Self::HardGate => "hard_gate",
            Self::SoftPositive => "soft_positive",
            Self::SoftNegative => "soft_negative",
            Self::Ignored => "ignored",
        }
    }

    pub const fn hard_gates(self) -> bool {
        matches!(self, Self::HardGate)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylonTrainingReputationProjection {
    pub namespace: PylonTrainingReputationNamespace,
    pub label: PylonTrainingReputationLabel,
    pub scheduler_effect: PylonTrainingSchedulerEffect,
    pub hard_gate: bool,
    pub age_days: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonTrainingObservabilityContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_pubkey: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub membership_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingAssignmentState {
    Planned,
    Leased,
    Acked,
    Active,
    Completed,
    Expired,
    Drained,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingContributionState {
    Received,
    Eligible,
    Sampled,
    Accepted,
    Quarantined,
    Rejected,
    ReplayRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingWindowState {
    Planned,
    Active,
    Sealing,
    Sealed,
    Validating,
    Accepted,
    Held,
    Refused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingValidatorChallengeState {
    Leased,
    Running,
    Retrying,
    Terminal,
    Held,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonTrainingRefusalCode {
    BadConfig,
    StaleAssignment,
    LeaseExpired,
    UnsupportedTopology,
    CheckpointMissing,
    CheckpointDigestMismatch,
    ArtifactIncomplete,
    ArtifactDigestMismatch,
    ValidatorTimeout,
    ValidatorDisagreement,
    SelfValidation,
    SelfPromotion,
    CheckpointChallengeWindowOpen,
    CheckpointQuorumMissing,
    EnvironmentMismatch,
    BuildRevoked,
}

impl PylonTrainingRefusalCode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::BadConfig => "bad_config",
            Self::StaleAssignment => "stale_assignment",
            Self::LeaseExpired => "lease_expired",
            Self::UnsupportedTopology => "unsupported_topology",
            Self::CheckpointMissing => "checkpoint_missing",
            Self::CheckpointDigestMismatch => "checkpoint_digest_mismatch",
            Self::ArtifactIncomplete => "artifact_incomplete",
            Self::ArtifactDigestMismatch => "artifact_digest_mismatch",
            Self::ValidatorTimeout => "validator_timeout",
            Self::ValidatorDisagreement => "validator_disagreement",
            Self::SelfValidation => "self_validation",
            Self::SelfPromotion => "self_promotion",
            Self::CheckpointChallengeWindowOpen => "checkpoint_challenge_window_open",
            Self::CheckpointQuorumMissing => "checkpoint_quorum_missing",
            Self::EnvironmentMismatch => "environment_mismatch",
            Self::BuildRevoked => "build_revoked",
        }
    }

    pub const fn owner(self) -> PylonTrainingAuthorityOwner {
        match self {
            Self::BadConfig
            | Self::UnsupportedTopology
            | Self::CheckpointMissing
            | Self::CheckpointDigestMismatch
            | Self::ArtifactIncomplete
            | Self::ArtifactDigestMismatch
            | Self::SelfValidation
            | Self::SelfPromotion
            | Self::EnvironmentMismatch => PylonTrainingAuthorityOwner::Pylon,
            Self::StaleAssignment
            | Self::LeaseExpired
            | Self::ValidatorTimeout
            | Self::ValidatorDisagreement
            | Self::CheckpointChallengeWindowOpen
            | Self::CheckpointQuorumMissing
            | Self::BuildRevoked => PylonTrainingAuthorityOwner::Nexus,
        }
    }

    pub const fn retryable(self) -> bool {
        match self {
            Self::LeaseExpired
            | Self::CheckpointMissing
            | Self::ArtifactIncomplete
            | Self::ValidatorTimeout
            | Self::CheckpointChallengeWindowOpen
            | Self::CheckpointQuorumMissing => true,
            Self::BadConfig
            | Self::StaleAssignment
            | Self::UnsupportedTopology
            | Self::CheckpointDigestMismatch
            | Self::ArtifactDigestMismatch
            | Self::ValidatorDisagreement
            | Self::SelfValidation
            | Self::SelfPromotion
            | Self::EnvironmentMismatch
            | Self::BuildRevoked => false,
        }
    }
}

fn require_non_empty(value: &str, field: &str) -> ContractResult<()> {
    if value.trim().is_empty() {
        return Err(format!("pylon_training_{field}_missing"));
    }
    Ok(())
}

fn require_prefixed_sha256(value: &str, field: &str) -> ContractResult<()> {
    if !value.starts_with("sha256:") || value.len() <= "sha256:".len() {
        return Err(format!("pylon_training_{field}_invalid"));
    }
    Ok(())
}

fn canonicalize_json_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(canonicalize_json_value)
                .collect::<Vec<_>>(),
        ),
        Value::Object(values) => {
            let sorted = values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json_value(value)))
                .collect::<BTreeMap<_, _>>();
            let mut map = Map::new();
            for (key, value) in sorted {
                map.insert(key, value);
            }
            Value::Object(map)
        }
        other => other,
    }
}

fn canonical_json_bytes_from_value(value: Value) -> ContractResult<Vec<u8>> {
    serde_json::to_vec(&canonicalize_json_value(value))
        .map_err(|error| format!("pylon_training_json_canonicalization_failed:{error}"))
}

pub fn canonical_json_sha256_digest<T: Serialize>(value: &T) -> ContractResult<String> {
    let value = serde_json::to_value(value)
        .map_err(|error| format!("pylon_training_json_serialize_failed:{error}"))?;
    Ok(sha256_prefixed_bytes(
        canonical_json_bytes_from_value(value)?.as_slice(),
    ))
}

pub fn pylon_training_membership_revision_label(membership_revision: u64) -> String {
    format!("members.rev{membership_revision}")
}

pub fn pylon_training_assignment_id(
    training_run_id: &str,
    window_id: &str,
    role_label: &str,
    slot_ordinal: u32,
    attempt: u32,
) -> String {
    format!("assign.{training_run_id}.{window_id}.{role_label}.{slot_ordinal}.attempt{attempt}")
}

pub fn pylon_training_lease_id(
    training_run_id: &str,
    window_id: &str,
    role_label: &str,
    slot_ordinal: u32,
    attempt: u32,
    membership_revision: u64,
) -> String {
    format!(
        "lease.{training_run_id}.{window_id}.{role_label}.{slot_ordinal}.attempt{attempt}.rev{membership_revision}"
    )
}

pub fn pylon_training_manifest_binding_digest(
    training_run_id: &str,
    window_id: &str,
    membership_revision: u64,
    node_pubkey_hex: &str,
    assignment_id: &str,
    role_label: &str,
    artifact_bucket_uri: &str,
) -> String {
    sha256_prefixed_bytes(
        format!(
            "{training_run_id}:{window_id}:{membership_revision}:{node_pubkey_hex}:{assignment_id}:{role_label}:{artifact_bucket_uri}"
        )
        .as_bytes(),
    )
}

pub fn pylon_training_dataset_identity_digest(
    dataset_slice: &ComputeAdapterDatasetSlice,
) -> ContractResult<String> {
    canonical_json_sha256_digest(&json!({
        "dataset_id": dataset_slice.dataset_id,
        "slice_id": dataset_slice.slice_id,
        "slice_digest": dataset_slice.slice_digest,
    }))
}

pub fn pylon_training_assignment_seed(
    training_run_id: &str,
    window_id: &str,
    membership_revision: &str,
    assignment_id: &str,
    node_pubkey_hex: &str,
    dataset_slice: &ComputeAdapterDatasetSlice,
) -> ContractResult<String> {
    canonical_json_sha256_digest(&json!({
        "training_run_id": training_run_id,
        "window_id": window_id,
        "membership_revision": membership_revision,
        "assignment_id": assignment_id,
        "node_pubkey_hex": node_pubkey_hex,
        "dataset_id": dataset_slice.dataset_id,
        "slice_id": dataset_slice.slice_id,
        "slice_digest": dataset_slice.slice_digest,
    }))
    .map_err(|_| "training_window_assignment_seed_encode_failed".to_string())
}

impl PylonTrainingTopology {
    pub fn validate_mvp(&self) -> ContractResult<()> {
        if self.world_size == 0 {
            return Err("pylon_training_topology_world_size_invalid".to_string());
        }
        if self.rank >= self.world_size {
            return Err("pylon_training_topology_rank_invalid".to_string());
        }
        if self.local_device_ids.is_empty() {
            return Err("pylon_training_topology_local_device_ids_missing".to_string());
        }
        if self
            .local_device_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != self.local_device_ids.len()
        {
            return Err("pylon_training_topology_local_device_ids_duplicate".to_string());
        }
        if self.backend_family == PylonTrainingTopologyBackendFamily::Mixed {
            return Err(PylonTrainingRefusalCode::UnsupportedTopology
                .label()
                .to_string());
        }
        if self.collective_kind != PylonTrainingCollectiveKind::DataParallel {
            return Err(PylonTrainingRefusalCode::UnsupportedTopology
                .label()
                .to_string());
        }
        if self.elastic_boundary != PylonTrainingElasticBoundary::Window {
            return Err(PylonTrainingRefusalCode::UnsupportedTopology
                .label()
                .to_string());
        }
        Ok(())
    }
}

impl PylonTrainingCheckpointBinding {
    fn validate(&self) -> ContractResult<()> {
        require_non_empty(self.checkpoint_family.as_str(), "checkpoint_family")?;
        require_non_empty(self.checkpoint_ref.as_str(), "checkpoint_ref")?;
        require_prefixed_sha256(self.manifest_digest.as_str(), "checkpoint_manifest_digest")?;
        require_non_empty(self.latest_pointer_ref.as_str(), "latest_pointer_ref")?;
        Ok(())
    }
}

impl PylonTrainingArtifactLayout {
    pub fn from_manifest(manifest: &PylonTrainingRunManifestV1) -> ContractResult<Self> {
        let layout = Self {
            bucket_uri: manifest.artifacts.bucket_uri.clone(),
            network_id: manifest.network_id.clone(),
            run_id: manifest.run_id.clone(),
            window_id: manifest.window_id.clone(),
        };
        layout.validate()?;
        Ok(layout)
    }

    pub fn validate(&self) -> ContractResult<()> {
        require_non_empty(self.bucket_uri.as_str(), "bucket_uri")?;
        if !self.bucket_uri.starts_with("gs://") {
            return Err("pylon_training_bucket_uri_invalid".to_string());
        }
        require_non_empty(self.network_id.as_str(), "network_id")?;
        require_non_empty(self.run_id.as_str(), "run_id")?;
        require_non_empty(self.window_id.as_str(), "window_id")?;
        Ok(())
    }

    fn normalized_bucket_uri(&self) -> String {
        self.bucket_uri.trim_end_matches('/').to_string()
    }

    pub fn run_prefix(&self) -> String {
        format!("networks/{}/runs/{}", self.network_id, self.run_id)
    }

    pub fn window_prefix(&self) -> String {
        format!("{}/windows/{}", self.run_prefix(), self.window_id)
    }

    pub fn run_root(&self) -> String {
        format!("{}/{}", self.normalized_bucket_uri(), self.run_prefix())
    }

    pub fn window_root(&self) -> String {
        format!("{}/{}", self.normalized_bucket_uri(), self.window_prefix())
    }

    pub fn run_manifest_path(&self) -> String {
        format!("{}/manifests/run_manifest.json", self.run_root())
    }

    pub fn latest_pointer_path(&self) -> String {
        format!("{}/checkpoints/latest_pointer.json", self.run_root())
    }

    pub fn checkpoint_manifest_path(&self, optimizer_step: u64) -> String {
        format!(
            "{}/checkpoints/step-{optimizer_step}/checkpoint_manifest.json",
            self.run_root()
        )
    }

    pub fn contribution_bundle_path(&self, assignment_id: &str) -> String {
        format!(
            "{}/contributions/{assignment_id}/adapter_delta_bundle.json",
            self.window_root()
        )
    }

    pub fn contribution_proof_bundle_path(&self, assignment_id: &str) -> String {
        format!(
            "{}/contributions/{assignment_id}/proof_bundle.json",
            self.window_root()
        )
    }

    pub fn validator_verdict_path(&self, challenge_id: &str) -> String {
        format!(
            "{}/validators/{challenge_id}/verdict.json",
            self.window_root()
        )
    }

    pub fn sealed_window_bundle_path(&self) -> String {
        format!("{}/sealed_window_bundle.json", self.window_root())
    }

    pub fn score_snapshot_path(&self) -> String {
        format!("{}/score_snapshot.json", self.window_root())
    }
}

impl PylonTrainingArtifacts {
    fn validate(&self, network_id: &str, run_id: &str, window_id: &str) -> ContractResult<()> {
        let layout = PylonTrainingArtifactLayout {
            bucket_uri: self.bucket_uri.clone(),
            network_id: network_id.to_string(),
            run_id: run_id.to_string(),
            window_id: window_id.to_string(),
        };
        layout.validate()?;
        if self.run_prefix != layout.run_prefix() {
            return Err("pylon_training_run_prefix_invalid".to_string());
        }
        if self.window_prefix != layout.window_prefix() {
            return Err("pylon_training_window_prefix_invalid".to_string());
        }
        require_non_empty(self.local_run_root.as_str(), "local_run_root")?;
        if self.credential_source != PYLON_TRAINING_GCS_CREDENTIAL_SOURCE {
            return Err("pylon_training_credential_source_invalid".to_string());
        }
        Ok(())
    }
}

impl PylonTrainingTrn {
    fn validate(&self) -> ContractResult<()> {
        require_non_empty(self.network_coordinate.as_str(), "network_coordinate")?;
        require_non_empty(self.window_coordinate.as_str(), "window_coordinate")?;
        if self.relay_urls.is_empty() {
            return Err("pylon_training_relay_urls_missing".to_string());
        }
        if self.relay_urls.iter().any(|value| value.trim().is_empty()) {
            return Err("pylon_training_relay_url_invalid".to_string());
        }
        Ok(())
    }
}

impl PylonTrainingDatasetAssignment {
    fn validate(&self) -> ContractResult<()> {
        require_non_empty(self.dataset_id.as_str(), "dataset_id")?;
        require_non_empty(self.slice_id.as_str(), "dataset_slice_id")?;
        require_prefixed_sha256(self.slice_digest.as_str(), "dataset_slice_digest")?;
        require_non_empty(self.assignment_seed.as_str(), "assignment_seed")?;
        Ok(())
    }
}

impl PylonTrainingValidatorAssignment {
    fn validate(&self) -> ContractResult<()> {
        require_non_empty(self.challenge_id.as_str(), "challenge_id")?;
        require_non_empty(self.challenge_kind.as_str(), "challenge_kind")?;
        if self.target_assignment_ids.is_empty() {
            return Err("pylon_training_validator_target_assignment_ids_missing".to_string());
        }
        if self.expected_manifest_digests.is_empty() {
            return Err("pylon_training_validator_expected_manifest_digests_missing".to_string());
        }
        for digest in &self.expected_manifest_digests {
            require_prefixed_sha256(digest.as_str(), "validator_expected_manifest_digest")?;
        }
        Ok(())
    }
}

impl PylonTrainingResumeFrom {
    fn validate(&self) -> ContractResult<()> {
        require_non_empty(self.checkpoint_ref.as_str(), "resume_checkpoint_ref")?;
        require_non_empty(
            self.latest_pointer_ref.as_str(),
            "resume_latest_pointer_ref",
        )?;
        require_prefixed_sha256(self.manifest_digest.as_str(), "resume_manifest_digest")?;
        Ok(())
    }
}

impl PylonTrainingRunManifestV1 {
    pub fn builder(
        role: PylonTrainingManifestRole,
        common: PylonTrainingRunManifestCommon,
        topology: PylonTrainingTopology,
        checkpoint: PylonTrainingCheckpointBinding,
        artifacts: PylonTrainingArtifacts,
        trn: PylonTrainingTrn,
    ) -> PylonTrainingRunManifestBuilder {
        PylonTrainingRunManifestBuilder {
            manifest: PylonTrainingRunManifestV1 {
                schema_version: PYLON_TRAINING_RUN_MANIFEST_V1.to_string(),
                manifest_id: common.manifest_id,
                manifest_digest: String::new(),
                issued_at_ms: common.issued_at_ms,
                expires_at_ms: common.expires_at_ms,
                network_id: common.network_id,
                run_id: common.run_id,
                window_id: common.window_id,
                assignment_id: common.assignment_id,
                lease_id: common.lease_id,
                lease_sequence: common.lease_sequence,
                membership_revision: common.membership_revision,
                role,
                node_pubkey: common.node_pubkey,
                coordinator_pubkey: common.coordinator_pubkey,
                authority_base_url: common.authority_base_url,
                training_policy_ref: common.training_policy_ref,
                validator_policy_ref: common.validator_policy_ref,
                environment_ref: common.environment_ref,
                environment_version: common.environment_version,
                execution_backend: PylonTrainingExecutionBackend::PsionicTrain,
                topology,
                checkpoint,
                artifacts,
                trn,
                dataset: None,
                validator: None,
                resume_from: None,
            },
        }
    }

    fn validate_core(&self, allow_empty_manifest_digest: bool) -> ContractResult<()> {
        if self.schema_version != PYLON_TRAINING_RUN_MANIFEST_V1 {
            return Err("pylon_training_schema_version_invalid".to_string());
        }
        require_non_empty(self.manifest_id.as_str(), "manifest_id")?;
        if !allow_empty_manifest_digest {
            require_prefixed_sha256(self.manifest_digest.as_str(), "manifest_digest")?;
        }
        if self.issued_at_ms == 0 {
            return Err("pylon_training_issued_at_ms_invalid".to_string());
        }
        if self.expires_at_ms <= self.issued_at_ms {
            return Err("pylon_training_expires_at_ms_invalid".to_string());
        }
        require_non_empty(self.network_id.as_str(), "network_id")?;
        require_non_empty(self.run_id.as_str(), "run_id")?;
        require_non_empty(self.window_id.as_str(), "window_id")?;
        require_non_empty(self.assignment_id.as_str(), "assignment_id")?;
        require_non_empty(self.lease_id.as_str(), "lease_id")?;
        if self.lease_sequence == 0 {
            return Err("pylon_training_lease_sequence_invalid".to_string());
        }
        require_non_empty(self.membership_revision.as_str(), "membership_revision")?;
        require_non_empty(self.node_pubkey.as_str(), "node_pubkey")?;
        require_non_empty(self.coordinator_pubkey.as_str(), "coordinator_pubkey")?;
        require_non_empty(self.authority_base_url.as_str(), "authority_base_url")?;
        if !(self.authority_base_url.starts_with("http://")
            || self.authority_base_url.starts_with("https://"))
        {
            return Err("pylon_training_authority_base_url_invalid".to_string());
        }
        require_non_empty(self.training_policy_ref.as_str(), "training_policy_ref")?;
        require_non_empty(self.validator_policy_ref.as_str(), "validator_policy_ref")?;
        require_non_empty(self.environment_ref.as_str(), "environment_ref")?;
        require_non_empty(self.environment_version.as_str(), "environment_version")?;
        if self.execution_backend != PylonTrainingExecutionBackend::PsionicTrain {
            return Err("pylon_training_execution_backend_invalid".to_string());
        }
        self.topology.validate_mvp()?;
        self.checkpoint.validate()?;
        self.artifacts.validate(
            self.network_id.as_str(),
            self.run_id.as_str(),
            self.window_id.as_str(),
        )?;
        self.trn.validate()?;
        match self.role {
            PylonTrainingManifestRole::Worker => {
                self.dataset
                    .as_ref()
                    .ok_or_else(|| "pylon_training_worker_dataset_missing".to_string())?
                    .validate()?;
                if self.validator.is_some() {
                    return Err("pylon_training_worker_validator_unexpected".to_string());
                }
                if let Some(resume_from) = self.resume_from.as_ref() {
                    resume_from.validate()?;
                }
            }
            PylonTrainingManifestRole::Validator => {
                self.validator
                    .as_ref()
                    .ok_or_else(|| "pylon_training_validator_section_missing".to_string())?
                    .validate()?;
                if let Some(dataset) = self.dataset.as_ref() {
                    dataset.validate()?;
                }
                if let Some(resume_from) = self.resume_from.as_ref() {
                    resume_from.validate()?;
                }
            }
            PylonTrainingManifestRole::RecoverySource => {
                if self.dataset.is_some() {
                    return Err("pylon_training_recovery_source_dataset_unexpected".to_string());
                }
                if self.validator.is_some() {
                    return Err("pylon_training_recovery_source_validator_unexpected".to_string());
                }
                self.resume_from
                    .as_ref()
                    .ok_or_else(|| "pylon_training_recovery_source_resume_missing".to_string())?
                    .validate()?;
            }
        }
        Ok(())
    }

    fn digest_basis_json(&self) -> ContractResult<Value> {
        let mut value = serde_json::to_value(self)
            .map_err(|error| format!("pylon_training_manifest_serialize_failed:{error}"))?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| "pylon_training_manifest_top_level_object_required".to_string())?;
        object.remove("manifest_digest");
        Ok(Value::Object(object.clone()))
    }

    pub fn canonical_digest(&self) -> ContractResult<String> {
        self.validate_core(true)?;
        Ok(sha256_prefixed_bytes(
            canonical_json_bytes_from_value(self.digest_basis_json()?)?.as_slice(),
        ))
    }

    pub fn canonical_json_bytes(&self) -> ContractResult<Vec<u8>> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| format!("pylon_training_manifest_serialize_failed:{error}"))?;
        canonical_json_bytes_from_value(value)
    }

    pub fn validate(&self) -> ContractResult<()> {
        self.validate_core(false)?;
        let expected_digest = self.canonical_digest()?;
        if self.manifest_digest != expected_digest {
            return Err("pylon_training_manifest_digest_mismatch".to_string());
        }
        Ok(())
    }
}

impl PylonTrainingRunManifestBuilder {
    pub fn dataset(mut self, dataset: PylonTrainingDatasetAssignment) -> Self {
        self.manifest.dataset = Some(dataset);
        self
    }

    pub fn validator(mut self, validator: PylonTrainingValidatorAssignment) -> Self {
        self.manifest.validator = Some(validator);
        self
    }

    pub fn resume_from(mut self, resume_from: PylonTrainingResumeFrom) -> Self {
        self.manifest.resume_from = Some(resume_from);
        self
    }

    pub fn build(mut self) -> ContractResult<PylonTrainingRunManifestV1> {
        self.manifest.validate_core(true)?;
        self.manifest.manifest_digest = self.manifest.canonical_digest()?;
        self.manifest.validate()?;
        Ok(self.manifest)
    }
}

pub fn parse_pylon_training_run_manifest_json(
    bytes: &[u8],
) -> ContractResult<PylonTrainingRunManifestV1> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("pylon_training_manifest_parse_failed:{error}"))?;
    if !value.is_object() {
        return Err("pylon_training_manifest_top_level_object_required".to_string());
    }
    let manifest: PylonTrainingRunManifestV1 = serde_json::from_value(value)
        .map_err(|error| format!("pylon_training_manifest_decode_failed:{error}"))?;
    manifest.validate()?;
    Ok(manifest)
}

pub fn validate_manifest_against_assignment_receipt(
    manifest: &PylonTrainingRunManifestV1,
    receipt: &PylonTrainingAssignmentPublishedReceipt,
) -> ContractResult<()> {
    manifest.validate()?;
    if receipt.kind != PYLON_TRAINING_TRN_ASSIGNMENT_RECEIPT_KIND {
        return Err("pylon_training_assignment_receipt_kind_invalid".to_string());
    }
    if receipt.assignment_id != manifest.assignment_id {
        return Err("pylon_training_assignment_receipt_assignment_id_mismatch".to_string());
    }
    if receipt.lease_id != manifest.lease_id {
        return Err("pylon_training_assignment_receipt_lease_id_mismatch".to_string());
    }
    if receipt.manifest_digest != manifest.manifest_digest {
        return Err("pylon_training_assignment_receipt_manifest_digest_mismatch".to_string());
    }
    Ok(())
}

impl PylonTrainingArtifactLocatorTags {
    pub fn validate(&self, manifest_backed: bool) -> ContractResult<()> {
        require_prefixed_sha256(self.x.as_str(), "artifact_locator_x")?;
        if manifest_backed {
            require_prefixed_sha256(
                self.manifest.as_deref().ok_or_else(|| {
                    "pylon_training_artifact_locator_manifest_missing".to_string()
                })?,
                "artifact_locator_manifest",
            )?;
        } else if let Some(manifest_digest) = self.manifest.as_ref() {
            require_prefixed_sha256(manifest_digest.as_str(), "artifact_locator_manifest")?;
        }
        Ok(())
    }
}

pub fn artifact_digest_from_bytes(bytes: &[u8]) -> String {
    sha256_prefixed_bytes(bytes)
}

pub fn artifact_digest_from_json<T: Serialize>(value: &T) -> ContractResult<String> {
    canonical_json_sha256_digest(value)
}

pub fn resolve_pylon_training_credentials(
    google_application_credentials: Option<&str>,
    metadata_available: bool,
) -> ContractResult<PylonTrainingCredentialResolution> {
    if google_application_credentials.is_some_and(|value| !value.trim().is_empty()) {
        return Ok(PylonTrainingCredentialResolution {
            persistent_credential_source: PYLON_TRAINING_GCS_CREDENTIAL_SOURCE.to_string(),
            resolution_method: PylonTrainingCredentialResolutionMethod::EnvironmentVariable,
        });
    }
    if metadata_available {
        return Ok(PylonTrainingCredentialResolution {
            persistent_credential_source: PYLON_TRAINING_GCS_CREDENTIAL_SOURCE.to_string(),
            resolution_method: PylonTrainingCredentialResolutionMethod::InstanceMetadata,
        });
    }
    Err("pylon_training_adc_credentials_unavailable".to_string())
}

pub fn validate_redacted_retained_state(value: &Value) -> ContractResult<()> {
    const DISALLOWED_KEYS: [&str; 9] = [
        "private_key",
        "private_key_id",
        "access_token",
        "refresh_token",
        "client_secret",
        "service_account",
        "service_account_json",
        "google_application_credentials_json",
        "authorization",
    ];

    fn visit(value: &Value, path: &str, disallowed: &[&str]) -> Option<String> {
        match value {
            Value::Object(entries) => entries.iter().find_map(|(key, value)| {
                let key_lc = key.to_ascii_lowercase();
                if disallowed.iter().any(|disallowed| key_lc == *disallowed) {
                    return Some(format!("{path}{key}"));
                }
                visit(value, format!("{path}{key}.").as_str(), disallowed)
            }),
            Value::Array(values) => values.iter().enumerate().find_map(|(index, value)| {
                visit(value, format!("{path}{index}.").as_str(), disallowed)
            }),
            Value::String(string) => {
                if let Ok(parsed) = serde_json::from_str::<Value>(string) {
                    if let Some(nested_path) = visit(&parsed, path, disallowed) {
                        return Some(nested_path);
                    }
                }
                if string.contains("\"private_key\"")
                    || string.contains("\"authorization\"")
                    || string.contains("-----BEGIN PRIVATE KEY-----")
                    || string.contains("Bearer ")
                {
                    Some(path.trim_end_matches('.').to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    if let Some(path) = visit(value, "", &DISALLOWED_KEYS) {
        return Err(format!(
            "pylon_training_retained_state_secret_detected:{path}"
        ));
    }
    Ok(())
}

pub fn validate_redacted_retained_content(content: &str) -> ContractResult<()> {
    if content.trim().is_empty() {
        return Ok(());
    }
    let parsed = serde_json::from_str::<Value>(content)
        .map_err(|error| format!("pylon_training_retained_state_content_parse_failed:{error}"))?;
    validate_redacted_retained_state(&parsed)
}

pub fn pylon_training_hard_gate_reason(labels: &[String]) -> Option<String> {
    fn label_parts(label: &str) -> (&str, &str) {
        let trimmed = label.trim();
        let mut segments = trimmed.split("::");
        let _subject = segments.next();
        let namespace = segments.next().unwrap_or_default().trim();
        let value = segments.next().map(str::trim).unwrap_or_else(|| {
            if let Some((_, suffix)) = trimmed.rsplit_once(':') {
                return suffix.trim();
            }
            if let Some((_, suffix)) = trimmed.rsplit_once('/') {
                return suffix.trim();
            }
            trimmed
        });
        (namespace, value)
    }

    let mut default_reason = None;
    for label in labels {
        let (namespace, value) = label_parts(label.as_str());
        if namespace == PylonTrainingReputationNamespace::Build.label()
            && value == PylonTrainingReputationLabel::Revoked.label()
        {
            return Some(PylonTrainingRefusalCode::BuildRevoked.label().to_string());
        }
        if matches!(value, "fraud" | "quarantined" | "revoked" | "inconsistent") {
            default_reason = Some("training_node_hard_gated".to_string());
        }
    }
    default_reason
}

impl PylonTrainingArtifactBundleKind {
    pub fn bundle_id(&self) -> String {
        match self {
            Self::RunManifest => "run_manifest".to_string(),
            Self::LatestCheckpointPointer => "latest_checkpoint_pointer".to_string(),
            Self::CheckpointManifest { optimizer_step } => {
                format!("checkpoint_manifest:{optimizer_step}")
            }
            Self::Contribution { assignment_id } => format!("contribution:{assignment_id}"),
            Self::ValidatorVerdict { challenge_id } => format!("validator_verdict:{challenge_id}"),
            Self::SealedWindow => "sealed_window".to_string(),
            Self::ScoreSnapshot => "score_snapshot".to_string(),
        }
    }

    pub fn bundle_kind_label(&self) -> &'static str {
        match self {
            Self::RunManifest => "run_manifest",
            Self::LatestCheckpointPointer => "latest_checkpoint_pointer",
            Self::CheckpointManifest { .. } => "checkpoint_manifest",
            Self::Contribution { .. } => "contribution",
            Self::ValidatorVerdict { .. } => "validator_verdict",
            Self::SealedWindow => "sealed_window",
            Self::ScoreSnapshot => "score_snapshot",
        }
    }

    pub fn required_paths(&self, layout: &PylonTrainingArtifactLayout) -> BTreeSet<String> {
        match self {
            Self::RunManifest => BTreeSet::from([layout.run_manifest_path()]),
            Self::LatestCheckpointPointer => BTreeSet::from([layout.latest_pointer_path()]),
            Self::CheckpointManifest { optimizer_step } => {
                BTreeSet::from([layout.checkpoint_manifest_path(*optimizer_step)])
            }
            Self::Contribution { assignment_id } => BTreeSet::from([
                layout.contribution_bundle_path(assignment_id.as_str()),
                layout.contribution_proof_bundle_path(assignment_id.as_str()),
            ]),
            Self::ValidatorVerdict { challenge_id } => {
                BTreeSet::from([layout.validator_verdict_path(challenge_id.as_str())])
            }
            Self::SealedWindow => BTreeSet::from([layout.sealed_window_bundle_path()]),
            Self::ScoreSnapshot => BTreeSet::from([layout.score_snapshot_path()]),
        }
    }
}

pub fn can_emit_terminal_artifact_uploaded_receipt(
    progress: &PylonTrainingArtifactBundleProgress,
) -> bool {
    !progress.required_objects.is_empty()
        && progress
            .required_objects
            .is_subset(&progress.uploaded_objects)
        && progress
            .required_objects
            .is_subset(&progress.digest_matched_objects)
}

pub fn derive_artifact_bundle_state(
    progress: &PylonTrainingArtifactBundleProgress,
) -> PylonTrainingArtifactBundleState {
    if progress.accepted {
        PylonTrainingArtifactBundleState::Accepted
    } else if progress.publication_succeeded {
        PylonTrainingArtifactBundleState::Published
    } else if progress.scheduler_verified {
        PylonTrainingArtifactBundleState::Verified
    } else if can_emit_terminal_artifact_uploaded_receipt(progress) {
        PylonTrainingArtifactBundleState::Uploaded
    } else if !progress.uploaded_objects.is_empty() || !progress.digest_matched_objects.is_empty() {
        PylonTrainingArtifactBundleState::Staged
    } else {
        PylonTrainingArtifactBundleState::LocalOnly
    }
}

pub fn validator_sample_count(admitted_contributions: u32) -> usize {
    let quarter_ceil = admitted_contributions.div_ceil(4) as usize;
    usize::min(8, usize::max(2, quarter_ceil))
}

pub fn validator_sample_assignments(
    window_id: &str,
    candidates: &[PylonTrainingContributionSampleCandidate],
) -> ContractResult<Vec<String>> {
    require_non_empty(window_id, "window_id")?;
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let sample_size = usize::min(
        validator_sample_count(candidates.len() as u32),
        candidates.len(),
    );
    let mut selected = Vec::new();
    let mut seen = BTreeSet::new();

    let highest_weight = candidates
        .iter()
        .max_by(|left, right| {
            left.aggregation_weight_bps
                .cmp(&right.aggregation_weight_bps)
                .then_with(|| right.assignment_id.cmp(&left.assignment_id))
        })
        .ok_or_else(|| "pylon_training_validator_sample_candidates_missing".to_string())?;
    if seen.insert(highest_weight.assignment_id.clone()) {
        selected.push(highest_weight.assignment_id.clone());
    }

    let earliest_assignment = candidates
        .iter()
        .min_by(|left, right| left.assignment_id.cmp(&right.assignment_id))
        .ok_or_else(|| "pylon_training_validator_sample_candidates_missing".to_string())?;
    if seen.insert(earliest_assignment.assignment_id.clone()) {
        selected.push(earliest_assignment.assignment_id.clone());
    }

    let mut remaining = candidates
        .iter()
        .filter(|candidate| !seen.contains(candidate.assignment_id.as_str()))
        .map(|candidate| {
            let mut hasher_input = Vec::new();
            hasher_input.extend_from_slice(window_id.as_bytes());
            hasher_input.extend_from_slice(candidate.assignment_id.as_bytes());
            (
                sha256::Hash::hash(hasher_input.as_slice()).to_string(),
                candidate.assignment_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    remaining.sort();

    for (_, assignment_id) in remaining {
        if selected.len() >= sample_size {
            break;
        }
        selected.push(assignment_id);
    }

    Ok(selected)
}

pub fn resolve_aggregate_verdicts(
    first: PylonTrainingContributionVerdict,
    second: Option<PylonTrainingContributionVerdict>,
) -> PylonTrainingAggregateResolution {
    match (first, second) {
        (PylonTrainingContributionVerdict::Accepted, None) => {
            PylonTrainingAggregateResolution::Accept
        }
        (
            PylonTrainingContributionVerdict::Accepted,
            Some(PylonTrainingContributionVerdict::Accepted),
        ) => PylonTrainingAggregateResolution::Accept,
        (PylonTrainingContributionVerdict::Accepted, Some(_)) => {
            PylonTrainingAggregateResolution::Held
        }
        (first, None) => {
            if matches!(
                first,
                PylonTrainingContributionVerdict::Quarantined
                    | PylonTrainingContributionVerdict::Rejected
                    | PylonTrainingContributionVerdict::ReplayRequired
            ) {
                PylonTrainingAggregateResolution::Escalate
            } else {
                PylonTrainingAggregateResolution::Held
            }
        }
        (first, Some(second)) if first == second => {
            PylonTrainingAggregateResolution::Terminal(first)
        }
        (_, Some(_)) => PylonTrainingAggregateResolution::Held,
    }
}

pub fn window_acceptance_ready(
    sampled_contributions_terminal: bool,
    aggregate_resolution: PylonTrainingAggregateResolution,
    held_challenge_present: bool,
) -> bool {
    sampled_contributions_terminal
        && !held_challenge_present
        && aggregate_resolution == PylonTrainingAggregateResolution::Accept
}

pub fn resolve_failure_ownership(
    scenario: PylonTrainingFailureScenario,
) -> PylonTrainingFailureResolution {
    match scenario {
        PylonTrainingFailureScenario::LeaseExpiryDuringUpload => PylonTrainingFailureResolution {
            owner: PylonTrainingAuthorityOwner::Nexus,
            state: "stale_upload_ignored",
            detail: "Nexus decides assignment freshness after lease expiry; stale uploads never become accepted window truth.",
        },
        PylonTrainingFailureScenario::PartialCheckpointPublish => PylonTrainingFailureResolution {
            owner: PylonTrainingAuthorityOwner::Pylon,
            state: "staged_only",
            detail: "Pylon owns local checkpoint upload completeness and cannot emit a terminal artifact_uploaded receipt for partial bundles.",
        },
        PylonTrainingFailureScenario::WorkerDrainDuringSeal => PylonTrainingFailureResolution {
            owner: PylonTrainingAuthorityOwner::Nexus,
            state: "membership_boundary_decision",
            detail: "Pylon can request drain, but Nexus decides whether the contribution lands in the current sealed window or the next membership revision.",
        },
        PylonTrainingFailureScenario::CrashAfterLocalCheckpointBeforeUpload => {
            PylonTrainingFailureResolution {
                owner: PylonTrainingAuthorityOwner::Nexus,
                state: "prior_durable_pointer_authoritative",
                detail: "Recovery continues from the last accepted durable checkpoint pointer rather than unpublished local state.",
            }
        }
        PylonTrainingFailureScenario::TrnPublicationFailure => PylonTrainingFailureResolution {
            owner: PylonTrainingAuthorityOwner::Nexus,
            state: "kernel_truth_retained_retry_publish",
            detail: "Local kernel truth stays authoritative while TRN publication retries asynchronously.",
        },
    }
}

fn validate_reputation_pair(
    namespace: PylonTrainingReputationNamespace,
    label: PylonTrainingReputationLabel,
) -> ContractResult<()> {
    let valid = matches!(
        (namespace, label),
        (
            PylonTrainingReputationNamespace::Contributor,
            PylonTrainingReputationLabel::Good
        ) | (
            PylonTrainingReputationNamespace::Contributor,
            PylonTrainingReputationLabel::Poor
        ) | (
            PylonTrainingReputationNamespace::Contributor,
            PylonTrainingReputationLabel::Quarantined
        ) | (
            PylonTrainingReputationNamespace::Contributor,
            PylonTrainingReputationLabel::Fraud
        ) | (
            PylonTrainingReputationNamespace::Validator,
            PylonTrainingReputationLabel::Good
        ) | (
            PylonTrainingReputationNamespace::Validator,
            PylonTrainingReputationLabel::Poor
        ) | (
            PylonTrainingReputationNamespace::Validator,
            PylonTrainingReputationLabel::Inconsistent
        ) | (
            PylonTrainingReputationNamespace::Build,
            PylonTrainingReputationLabel::Admitted
        ) | (
            PylonTrainingReputationNamespace::Build,
            PylonTrainingReputationLabel::Stale
        ) | (
            PylonTrainingReputationNamespace::Build,
            PylonTrainingReputationLabel::Revoked
        ) | (
            PylonTrainingReputationNamespace::Checkpoint,
            PylonTrainingReputationLabel::Warning
        ) | (
            PylonTrainingReputationNamespace::Checkpoint,
            PylonTrainingReputationLabel::Revoked
        )
    );
    if !valid {
        return Err("pylon_training_reputation_label_invalid".to_string());
    }
    Ok(())
}

impl PylonTrainingReputationRecord {
    pub fn new(
        namespace: PylonTrainingReputationNamespace,
        label: PylonTrainingReputationLabel,
        subject_pubkey: Option<String>,
        event_ref: Option<String>,
        address_ref: Option<String>,
    ) -> ContractResult<Self> {
        validate_reputation_pair(namespace, label)?;
        Ok(Self {
            authority_owner: PylonTrainingAuthorityOwner::Nexus,
            namespace,
            label,
            subject_pubkey,
            event_ref,
            address_ref,
        })
    }

    pub fn validate(&self) -> ContractResult<()> {
        if self.authority_owner != PylonTrainingAuthorityOwner::Nexus {
            return Err("pylon_training_reputation_authority_invalid".to_string());
        }
        validate_reputation_pair(self.namespace, self.label)?;
        if self.subject_pubkey.is_none() && self.event_ref.is_none() && self.address_ref.is_none() {
            return Err("pylon_training_reputation_target_missing".to_string());
        }
        Ok(())
    }
}

pub fn scheduler_effect_for_label(
    namespace: PylonTrainingReputationNamespace,
    label: PylonTrainingReputationLabel,
    age_days: u32,
) -> ContractResult<PylonTrainingSchedulerEffect> {
    validate_reputation_pair(namespace, label)?;
    let effect = match label {
        PylonTrainingReputationLabel::Fraud | PylonTrainingReputationLabel::Revoked => {
            PylonTrainingSchedulerEffect::HardGate
        }
        PylonTrainingReputationLabel::Good => {
            if age_days > 30 {
                PylonTrainingSchedulerEffect::Ignored
            } else {
                PylonTrainingSchedulerEffect::SoftPositive
            }
        }
        PylonTrainingReputationLabel::Poor
        | PylonTrainingReputationLabel::Stale
        | PylonTrainingReputationLabel::Warning => {
            if age_days > 30 {
                PylonTrainingSchedulerEffect::Ignored
            } else {
                PylonTrainingSchedulerEffect::SoftNegative
            }
        }
        PylonTrainingReputationLabel::Quarantined | PylonTrainingReputationLabel::Inconsistent => {
            if age_days <= 7 {
                PylonTrainingSchedulerEffect::HardGate
            } else if age_days <= 30 {
                PylonTrainingSchedulerEffect::SoftNegative
            } else {
                PylonTrainingSchedulerEffect::Ignored
            }
        }
        PylonTrainingReputationLabel::Admitted => PylonTrainingSchedulerEffect::SoftPositive,
    };
    Ok(effect)
}

pub fn reputation_projection_for_label(
    namespace: PylonTrainingReputationNamespace,
    label: PylonTrainingReputationLabel,
    created_at_unix: u64,
    now_unix: u64,
) -> ContractResult<PylonTrainingReputationProjection> {
    let age_secs = now_unix.saturating_sub(created_at_unix);
    let age_days = u32::try_from(age_secs / 86_400).unwrap_or(u32::MAX);
    let scheduler_effect = scheduler_effect_for_label(namespace, label, age_days)?;
    Ok(PylonTrainingReputationProjection {
        namespace,
        label,
        hard_gate: scheduler_effect.hard_gates(),
        scheduler_effect,
        age_days,
    })
}

impl PylonTrainingObservabilityContext {
    pub fn validate(&self) -> ContractResult<()> {
        const PLACEHOLDERS: [&str; 4] = ["unknown", "placeholder", "uninitialized", "n/a"];

        for (field, value) in [
            ("network_id", self.network_id.as_deref()),
            ("run_id", self.run_id.as_deref()),
            ("window_id", self.window_id.as_deref()),
            ("assignment_id", self.assignment_id.as_deref()),
            ("challenge_id", self.challenge_id.as_deref()),
            ("node_pubkey", self.node_pubkey.as_deref()),
            ("membership_revision", self.membership_revision.as_deref()),
            ("manifest_digest", self.manifest_digest.as_deref()),
        ] {
            if let Some(value) = value {
                require_non_empty(value, field)?;
                if PLACEHOLDERS
                    .iter()
                    .any(|placeholder| value.eq_ignore_ascii_case(placeholder))
                {
                    return Err(format!("pylon_training_observability_{field}_placeholder"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest_common() -> PylonTrainingRunManifestCommon {
        PylonTrainingRunManifestCommon {
            manifest_id: "manifest.run.alpha.worker.node01.lease03".to_string(),
            issued_at_ms: 1_762_670_000_000,
            expires_at_ms: 1_762_670_600_000,
            network_id: "trainnet.alpha".to_string(),
            run_id: "run.alpha".to_string(),
            window_id: "window.000123".to_string(),
            assignment_id: "assign.node01.window000123".to_string(),
            lease_id: "lease.node01.window000123".to_string(),
            lease_sequence: 3,
            membership_revision: "members.rev7".to_string(),
            node_pubkey: "0123abcd".to_string(),
            coordinator_pubkey: "0456efgh".to_string(),
            authority_base_url: "https://nexus.example".to_string(),
            training_policy_ref: "policy://training/adapter/v1".to_string(),
            validator_policy_ref: "policy://validator/mvp/v1".to_string(),
            environment_ref: "env.openagents.cuda.train".to_string(),
            environment_version: "2026.04.09".to_string(),
        }
    }

    fn topology() -> PylonTrainingTopology {
        PylonTrainingTopology {
            backend_family: PylonTrainingTopologyBackendFamily::Cuda,
            world_size: 4,
            rank: 1,
            local_device_ids: vec![1],
            collective_kind: PylonTrainingCollectiveKind::DataParallel,
            elastic_boundary: PylonTrainingElasticBoundary::Window,
        }
    }

    fn checkpoint() -> PylonTrainingCheckpointBinding {
        PylonTrainingCheckpointBinding {
            checkpoint_family: "adapter.reference.open".to_string(),
            checkpoint_ref: "checkpoint://adapter/reference/base".to_string(),
            manifest_digest: "sha256:checkpoint-manifest".to_string(),
            latest_pointer_ref:
                "gs://bucket/networks/trainnet.alpha/runs/run.alpha/checkpoints/latest_pointer.json"
                    .to_string(),
        }
    }

    fn artifacts() -> PylonTrainingArtifacts {
        PylonTrainingArtifacts {
            bucket_uri: "gs://bucket".to_string(),
            run_prefix: "networks/trainnet.alpha/runs/run.alpha".to_string(),
            window_prefix: "networks/trainnet.alpha/runs/run.alpha/windows/window.000123"
                .to_string(),
            local_run_root: "/var/lib/openagents/pylon/runs/run.alpha".to_string(),
            credential_source: PYLON_TRAINING_GCS_CREDENTIAL_SOURCE.to_string(),
        }
    }

    fn trn() -> PylonTrainingTrn {
        PylonTrainingTrn {
            network_coordinate: "39500:coordinator:trainnet.alpha".to_string(),
            window_coordinate: "39510:coordinator:window.000123".to_string(),
            relay_urls: vec!["wss://relay.one".to_string(), "wss://relay.two".to_string()],
        }
    }

    fn dataset_slice() -> ComputeAdapterDatasetSlice {
        ComputeAdapterDatasetSlice {
            dataset_id: "dataset://train/reference".to_string(),
            split_name: "train".to_string(),
            slice_id: "slice://000123".to_string(),
            slice_digest: "sha256:dataset-slice".to_string(),
        }
    }

    fn worker_manifest() -> PylonTrainingRunManifestV1 {
        let dataset_slice = dataset_slice();
        PylonTrainingRunManifestV1::builder(
            PylonTrainingManifestRole::Worker,
            manifest_common(),
            topology(),
            checkpoint(),
            artifacts(),
            trn(),
        )
        .dataset(PylonTrainingDatasetAssignment {
            dataset_id: dataset_slice.dataset_id.clone(),
            slice_id: dataset_slice.slice_id.clone(),
            slice_digest: dataset_slice.slice_digest.clone(),
            assignment_seed: pylon_training_assignment_seed(
                "run.alpha",
                "window.000123",
                "members.rev7",
                "assign.node01.window000123",
                "0123abcd",
                &dataset_slice,
            )
            .expect("assignment seed"),
        })
        .build()
        .expect("worker manifest should build")
    }

    #[test]
    fn manifest_builder_round_trips_and_stabilizes_digest() {
        let manifest = worker_manifest();
        let canonical_bytes = manifest
            .canonical_json_bytes()
            .expect("canonical bytes should render");
        let reparsed =
            parse_pylon_training_run_manifest_json(canonical_bytes.as_slice()).expect("reparse");
        assert_eq!(reparsed, manifest);
        assert_eq!(
            manifest.manifest_digest,
            manifest.canonical_digest().expect("digest should compute")
        );
    }

    #[test]
    fn manifest_parser_ignores_unknown_additive_fields() {
        let mut value = serde_json::to_value(worker_manifest()).expect("serialize manifest");
        value["future_field"] = json!({"new": true});
        let parsed = parse_pylon_training_run_manifest_json(
            serde_json::to_vec(&value)
                .expect("manifest json")
                .as_slice(),
        )
        .expect("parser should ignore unknown additive fields");
        assert_eq!(parsed.role, PylonTrainingManifestRole::Worker);
    }

    #[test]
    fn manifest_role_requirements_are_enforced() {
        let err = PylonTrainingRunManifestV1::builder(
            PylonTrainingManifestRole::Worker,
            manifest_common(),
            topology(),
            checkpoint(),
            artifacts(),
            trn(),
        )
        .build()
        .expect_err("worker manifest without dataset should fail");
        assert_eq!(err, "pylon_training_worker_dataset_missing");

        let err = PylonTrainingRunManifestV1::builder(
            PylonTrainingManifestRole::Validator,
            manifest_common(),
            topology(),
            checkpoint(),
            artifacts(),
            trn(),
        )
        .build()
        .expect_err("validator manifest without validator section should fail");
        assert_eq!(err, "pylon_training_validator_section_missing");

        let err = PylonTrainingRunManifestV1::builder(
            PylonTrainingManifestRole::RecoverySource,
            manifest_common(),
            topology(),
            checkpoint(),
            artifacts(),
            trn(),
        )
        .build()
        .expect_err("recovery-source manifest without resume_from should fail");
        assert_eq!(err, "pylon_training_recovery_source_resume_missing");

        let coordinator_json = json!({
            "schema_version": PYLON_TRAINING_RUN_MANIFEST_V1,
            "manifest_id": "manifest.invalid",
            "manifest_digest": "sha256:placeholder",
            "issued_at_ms": 1,
            "expires_at_ms": 2,
            "network_id": "trainnet.alpha",
            "run_id": "run.alpha",
            "window_id": "window.000123",
            "assignment_id": "assign.node01.window000123",
            "lease_id": "lease.node01.window000123",
            "lease_sequence": 1,
            "membership_revision": "members.rev7",
            "role": "coordinator",
            "node_pubkey": "0123",
            "coordinator_pubkey": "0456",
            "authority_base_url": "https://nexus.example",
            "training_policy_ref": "policy://training/adapter/v1",
            "validator_policy_ref": "policy://validator/mvp/v1",
            "environment_ref": "env.openagents.cuda.train",
            "environment_version": "2026.04.09",
            "execution_backend": "psionic_train",
            "topology": {
                "backend_family": "cuda",
                "world_size": 1,
                "rank": 0,
                "local_device_ids": [0],
                "collective_kind": "data_parallel",
                "elastic_boundary": "window"
            },
            "checkpoint": checkpoint(),
            "artifacts": artifacts(),
            "trn": trn(),
            "dataset": {
                "dataset_id": "dataset://train/reference",
                "slice_id": "slice://000123",
                "slice_digest": "sha256:dataset-slice",
                "assignment_seed": "seed://window.000123/node01"
            }
        });
        let err = parse_pylon_training_run_manifest_json(
            serde_json::to_vec(&coordinator_json)
                .expect("json")
                .as_slice(),
        )
        .expect_err("coordinator manifests are not accepted");
        assert!(err.starts_with("pylon_training_manifest_decode_failed"));
    }

    #[test]
    fn topology_validator_allows_homogeneous_backends_and_rejects_mixed_or_non_window_elasticity() {
        let mut allowed = topology();
        allowed.backend_family = PylonTrainingTopologyBackendFamily::Mlx;
        allowed
            .validate_mvp()
            .expect("mlx backend should stay admitted for homogeneous Apple windows");

        let mut allowed = topology();
        allowed.backend_family = PylonTrainingTopologyBackendFamily::Metal;
        allowed
            .validate_mvp()
            .expect("metal backend should stay admitted for homogeneous Apple windows");

        let mut invalid = topology();
        invalid.backend_family = PylonTrainingTopologyBackendFamily::Mixed;
        let err = invalid
            .validate_mvp()
            .expect_err("mixed backend should be rejected");
        assert_eq!(err, "unsupported_topology");

        let mut invalid = topology();
        invalid.collective_kind = PylonTrainingCollectiveKind::TensorParallel;
        let err = invalid
            .validate_mvp()
            .expect_err("tensor parallel should be rejected");
        assert_eq!(err, "unsupported_topology");

        let mut invalid = topology();
        invalid.elastic_boundary = PylonTrainingElasticBoundary::MidWindow;
        let err = invalid
            .validate_mvp()
            .expect_err("mid-window elasticity should be rejected");
        assert_eq!(err, "unsupported_topology");
    }

    #[test]
    fn assignment_receipt_cross_check_matches_manifest_digest() {
        let manifest = worker_manifest();
        let receipt = PylonTrainingAssignmentPublishedReceipt {
            kind: PYLON_TRAINING_TRN_ASSIGNMENT_RECEIPT_KIND,
            assignment_id: manifest.assignment_id.clone(),
            lease_id: manifest.lease_id.clone(),
            manifest_digest: manifest.manifest_digest.clone(),
        };
        validate_manifest_against_assignment_receipt(&manifest, &receipt)
            .expect("matching receipt should validate");

        let mismatch = PylonTrainingAssignmentPublishedReceipt {
            manifest_digest: "sha256:nope".to_string(),
            ..receipt
        };
        let err = validate_manifest_against_assignment_receipt(&manifest, &mismatch)
            .expect_err("mismatch should be rejected");
        assert_eq!(
            err,
            "pylon_training_assignment_receipt_manifest_digest_mismatch"
        );
    }

    #[test]
    fn gcs_artifact_layout_covers_every_required_object_path() {
        let layout = PylonTrainingArtifactLayout {
            bucket_uri: "gs://bucket".to_string(),
            network_id: "trainnet.alpha".to_string(),
            run_id: "run.alpha".to_string(),
            window_id: "window.000123".to_string(),
        };
        assert_eq!(
            layout.run_manifest_path(),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha/manifests/run_manifest.json"
        );
        assert_eq!(
            layout.latest_pointer_path(),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha/checkpoints/latest_pointer.json"
        );
        assert_eq!(
            layout.checkpoint_manifest_path(42),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha/checkpoints/step-42/checkpoint_manifest.json"
        );
        assert_eq!(
            layout.contribution_bundle_path("assign.node01.window000123"),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha/windows/window.000123/contributions/assign.node01.window000123/adapter_delta_bundle.json"
        );
        assert_eq!(
            layout.contribution_proof_bundle_path("assign.node01.window000123"),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha/windows/window.000123/contributions/assign.node01.window000123/proof_bundle.json"
        );
        assert_eq!(
            layout.validator_verdict_path("challenge.alpha"),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha/windows/window.000123/validators/challenge.alpha/verdict.json"
        );
        assert_eq!(
            layout.sealed_window_bundle_path(),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha/windows/window.000123/sealed_window_bundle.json"
        );
        assert_eq!(
            layout.score_snapshot_path(),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha/windows/window.000123/score_snapshot.json"
        );
    }

    #[test]
    fn shared_training_identity_helpers_match_frozen_contracts() {
        let dataset_slice = dataset_slice();
        assert_eq!(pylon_training_membership_revision_label(7), "members.rev7");
        assert_eq!(
            pylon_training_assignment_id("run.alpha", "window.000123", "worker", 1, 2),
            "assign.run.alpha.window.000123.worker.1.attempt2"
        );
        assert_eq!(
            pylon_training_lease_id("run.alpha", "window.000123", "worker", 1, 2, 7),
            "lease.run.alpha.window.000123.worker.1.attempt2.rev7"
        );
        assert_eq!(
            pylon_training_manifest_binding_digest(
                "run.alpha",
                "window.000123",
                7,
                "0123abcd",
                "assign.run.alpha.window.000123.worker.1.attempt2",
                "worker",
                "gs://bucket",
            ),
            sha256_prefixed_bytes(
                b"run.alpha:window.000123:7:0123abcd:assign.run.alpha.window.000123.worker.1.attempt2:worker:gs://bucket",
            )
        );
        assert_eq!(
            pylon_training_dataset_identity_digest(&dataset_slice).expect("dataset identity"),
            canonical_json_sha256_digest(&json!({
                "dataset_id": "dataset://train/reference",
                "slice_id": "slice://000123",
                "slice_digest": "sha256:dataset-slice",
            }))
            .expect("dataset identity digest")
        );
        assert_eq!(
            pylon_training_assignment_seed(
                "run.alpha",
                "window.000123",
                "members.rev7",
                "assign.run.alpha.window.000123.worker.1.attempt2",
                "0123abcd",
                &dataset_slice,
            )
            .expect("assignment seed"),
            canonical_json_sha256_digest(&json!({
                "training_run_id": "run.alpha",
                "window_id": "window.000123",
                "membership_revision": "members.rev7",
                "assignment_id": "assign.run.alpha.window.000123.worker.1.attempt2",
                "node_pubkey_hex": "0123abcd",
                "dataset_id": "dataset://train/reference",
                "slice_id": "slice://000123",
                "slice_digest": "sha256:dataset-slice",
            }))
            .expect("assignment seed digest")
        );
    }

    #[test]
    fn artifact_layout_and_bundle_helpers_match_frozen_contracts() {
        let manifest = worker_manifest();
        let layout = PylonTrainingArtifactLayout::from_manifest(&manifest).expect("layout");
        assert_eq!(
            layout.run_root(),
            "gs://bucket/networks/trainnet.alpha/runs/run.alpha"
        );
        assert_eq!(
            PylonTrainingArtifactBundleKind::RunManifest.bundle_id(),
            "run_manifest"
        );
        assert_eq!(
            PylonTrainingArtifactBundleKind::LatestCheckpointPointer.bundle_id(),
            "latest_checkpoint_pointer"
        );
        assert_eq!(
            PylonTrainingArtifactBundleKind::CheckpointManifest { optimizer_step: 42 }.bundle_id(),
            "checkpoint_manifest:42"
        );
        assert_eq!(
            PylonTrainingArtifactBundleKind::Contribution {
                assignment_id: "assign.node01.window000123".to_string()
            }
            .bundle_id(),
            "contribution:assign.node01.window000123"
        );
        assert_eq!(
            PylonTrainingArtifactBundleKind::ValidatorVerdict {
                challenge_id: "challenge.alpha".to_string()
            }
            .bundle_id(),
            "validator_verdict:challenge.alpha"
        );
        assert_eq!(
            PylonTrainingArtifactBundleKind::SealedWindow.bundle_kind_label(),
            "sealed_window"
        );
        assert_eq!(
            PylonTrainingArtifactBundleKind::ScoreSnapshot.bundle_kind_label(),
            "score_snapshot"
        );
    }

    #[test]
    fn digest_policy_and_locator_tags_match_the_frozen_contract() {
        let digest = artifact_digest_from_bytes(b"hello");
        assert!(digest.starts_with("sha256:"));
        let manifest_digest = artifact_digest_from_json(&worker_manifest()).expect("json digest");
        assert!(manifest_digest.starts_with("sha256:"));
        PylonTrainingArtifactLocatorTags {
            x: digest,
            manifest: Some(manifest_digest),
        }
        .validate(true)
        .expect("manifest-backed locator should validate");

        let err = PylonTrainingArtifactLocatorTags {
            x: "sha256:file".to_string(),
            manifest: None,
        }
        .validate(true)
        .expect_err("manifest-backed locator without manifest digest should fail");
        assert_eq!(err, "pylon_training_artifact_locator_manifest_missing");
    }

    #[test]
    fn adc_resolution_prefers_env_then_metadata_and_redacts_retained_state() {
        let env_resolution =
            resolve_pylon_training_credentials(Some("/tmp/service-account.json"), false)
                .expect("env ADC should resolve");
        assert_eq!(
            env_resolution.resolution_method,
            PylonTrainingCredentialResolutionMethod::EnvironmentVariable
        );
        assert_eq!(
            env_resolution.persistent_credential_source,
            PYLON_TRAINING_GCS_CREDENTIAL_SOURCE
        );

        let metadata_resolution =
            resolve_pylon_training_credentials(None, true).expect("metadata ADC should resolve");
        assert_eq!(
            metadata_resolution.resolution_method,
            PylonTrainingCredentialResolutionMethod::InstanceMetadata
        );

        validate_redacted_retained_state(&json!({
            "credential_source": PYLON_TRAINING_GCS_CREDENTIAL_SOURCE
        }))
        .expect("credential source name may be retained");
        validate_redacted_retained_content(
            json!({
                "credential_source": PYLON_TRAINING_GCS_CREDENTIAL_SOURCE
            })
            .to_string()
            .as_str(),
        )
        .expect("JSON content without secret fields may be retained");
        validate_redacted_retained_content("")
            .expect("empty retained content remains valid for label events");

        let err = validate_redacted_retained_state(&json!({
            "credential_source": PYLON_TRAINING_GCS_CREDENTIAL_SOURCE,
            "private_key": "secret"
        }))
        .expect_err("secret fields must be rejected");
        assert_eq!(
            err,
            "pylon_training_retained_state_secret_detected:private_key"
        );
        let err = validate_redacted_retained_content(
            json!({
                "authorization": "Bearer secret"
            })
            .to_string()
            .as_str(),
        )
        .expect_err("secret-bearing JSON content must be rejected");
        assert_eq!(
            err,
            "pylon_training_retained_state_secret_detected:authorization"
        );
        let err = validate_redacted_retained_state(&json!({
            "publication_records": {
                "receipt.alpha": {
                    "template": {
                        "content": json!({
                            "authorization": "Bearer secret"
                        })
                        .to_string()
                    }
                }
            }
        }))
        .expect_err("nested stringified retained content must be rejected");
        assert_eq!(
            err,
            "pylon_training_retained_state_secret_detected:publication_records.receipt.alpha.template.content.authorization"
        );
    }

    #[test]
    fn partial_bundle_never_emits_terminal_upload_success() {
        let layout = PylonTrainingArtifactLayout {
            bucket_uri: "gs://bucket".to_string(),
            network_id: "trainnet.alpha".to_string(),
            run_id: "run.alpha".to_string(),
            window_id: "window.000123".to_string(),
        };
        let required = PylonTrainingArtifactBundleKind::Contribution {
            assignment_id: "assign.node01.window000123".to_string(),
        }
        .required_paths(&layout);
        let only_one_uploaded =
            BTreeSet::from([layout.contribution_bundle_path("assign.node01.window000123")]);
        let partial = PylonTrainingArtifactBundleProgress {
            required_objects: required.clone(),
            uploaded_objects: only_one_uploaded.clone(),
            digest_matched_objects: only_one_uploaded,
            scheduler_verified: false,
            publication_succeeded: false,
            accepted: false,
        };
        assert!(!can_emit_terminal_artifact_uploaded_receipt(&partial));
        assert_eq!(
            derive_artifact_bundle_state(&partial),
            PylonTrainingArtifactBundleState::Staged
        );

        let complete = PylonTrainingArtifactBundleProgress {
            required_objects: required.clone(),
            uploaded_objects: required.clone(),
            digest_matched_objects: required,
            scheduler_verified: true,
            publication_succeeded: true,
            accepted: true,
        };
        assert!(can_emit_terminal_artifact_uploaded_receipt(&complete));
        assert_eq!(
            derive_artifact_bundle_state(&complete),
            PylonTrainingArtifactBundleState::Accepted
        );
    }

    #[test]
    fn validator_sampling_and_escalation_follow_the_frozen_policy() {
        let candidates = vec![
            PylonTrainingContributionSampleCandidate {
                assignment_id: "assign-c".to_string(),
                aggregation_weight_bps: 6_000,
            },
            PylonTrainingContributionSampleCandidate {
                assignment_id: "assign-a".to_string(),
                aggregation_weight_bps: 9_000,
            },
            PylonTrainingContributionSampleCandidate {
                assignment_id: "assign-b".to_string(),
                aggregation_weight_bps: 4_000,
            },
            PylonTrainingContributionSampleCandidate {
                assignment_id: "assign-d".to_string(),
                aggregation_weight_bps: 3_000,
            },
        ];
        assert_eq!(validator_sample_count(1), 2);
        assert_eq!(validator_sample_count(8), 2);
        assert_eq!(validator_sample_count(12), 3);
        let sample =
            validator_sample_assignments("window.000123", &candidates).expect("sample set");
        assert_eq!(sample.len(), 2);
        assert_eq!(sample[0], "assign-a");
        assert_eq!(sample[1], "assign-b");

        assert_eq!(
            resolve_aggregate_verdicts(PylonTrainingContributionVerdict::Accepted, None),
            PylonTrainingAggregateResolution::Accept
        );
        assert_eq!(
            resolve_aggregate_verdicts(PylonTrainingContributionVerdict::Rejected, None),
            PylonTrainingAggregateResolution::Escalate
        );
        assert_eq!(
            resolve_aggregate_verdicts(
                PylonTrainingContributionVerdict::Rejected,
                Some(PylonTrainingContributionVerdict::Rejected)
            ),
            PylonTrainingAggregateResolution::Terminal(PylonTrainingContributionVerdict::Rejected)
        );
        assert_eq!(
            resolve_aggregate_verdicts(
                PylonTrainingContributionVerdict::Rejected,
                Some(PylonTrainingContributionVerdict::Quarantined)
            ),
            PylonTrainingAggregateResolution::Held
        );
        assert!(window_acceptance_ready(
            true,
            PylonTrainingAggregateResolution::Accept,
            false
        ));
        assert!(!window_acceptance_ready(
            true,
            PylonTrainingAggregateResolution::Held,
            true
        ));
    }

    #[test]
    fn failure_ownership_matches_the_frozen_boundary() {
        assert_eq!(
            resolve_failure_ownership(PylonTrainingFailureScenario::LeaseExpiryDuringUpload).owner,
            PylonTrainingAuthorityOwner::Nexus
        );
        assert_eq!(
            resolve_failure_ownership(PylonTrainingFailureScenario::PartialCheckpointPublish).owner,
            PylonTrainingAuthorityOwner::Pylon
        );
        assert_eq!(
            resolve_failure_ownership(PylonTrainingFailureScenario::WorkerDrainDuringSeal).owner,
            PylonTrainingAuthorityOwner::Nexus
        );
        assert_eq!(
            resolve_failure_ownership(
                PylonTrainingFailureScenario::CrashAfterLocalCheckpointBeforeUpload
            )
            .state,
            "prior_durable_pointer_authoritative"
        );
    }

    #[test]
    fn reputation_projection_covers_every_frozen_label_class() {
        let cases = [
            (
                PylonTrainingReputationNamespace::Contributor,
                PylonTrainingReputationLabel::Good,
                3,
                PylonTrainingSchedulerEffect::SoftPositive,
            ),
            (
                PylonTrainingReputationNamespace::Contributor,
                PylonTrainingReputationLabel::Poor,
                3,
                PylonTrainingSchedulerEffect::SoftNegative,
            ),
            (
                PylonTrainingReputationNamespace::Contributor,
                PylonTrainingReputationLabel::Quarantined,
                3,
                PylonTrainingSchedulerEffect::HardGate,
            ),
            (
                PylonTrainingReputationNamespace::Contributor,
                PylonTrainingReputationLabel::Fraud,
                45,
                PylonTrainingSchedulerEffect::HardGate,
            ),
            (
                PylonTrainingReputationNamespace::Validator,
                PylonTrainingReputationLabel::Inconsistent,
                10,
                PylonTrainingSchedulerEffect::SoftNegative,
            ),
            (
                PylonTrainingReputationNamespace::Build,
                PylonTrainingReputationLabel::Admitted,
                1,
                PylonTrainingSchedulerEffect::SoftPositive,
            ),
            (
                PylonTrainingReputationNamespace::Build,
                PylonTrainingReputationLabel::Stale,
                3,
                PylonTrainingSchedulerEffect::SoftNegative,
            ),
            (
                PylonTrainingReputationNamespace::Build,
                PylonTrainingReputationLabel::Revoked,
                1,
                PylonTrainingSchedulerEffect::HardGate,
            ),
            (
                PylonTrainingReputationNamespace::Checkpoint,
                PylonTrainingReputationLabel::Warning,
                3,
                PylonTrainingSchedulerEffect::SoftNegative,
            ),
        ];
        for (namespace, label, age_days, expected_effect) in cases {
            let record = PylonTrainingReputationRecord::new(
                namespace,
                label,
                Some("node-pubkey".to_string()),
                Some("event-id".to_string()),
                None,
            )
            .expect("record should build");
            record.validate().expect("record should validate");
            assert_eq!(
                scheduler_effect_for_label(namespace, label, age_days)
                    .expect("effect should resolve"),
                expected_effect
            );
        }
    }

    #[test]
    fn reputation_enums_round_trip_frozen_labels() {
        for (namespace, label) in [
            (
                PylonTrainingReputationNamespace::Contributor,
                "trn/contributor",
            ),
            (PylonTrainingReputationNamespace::Validator, "trn/validator"),
            (PylonTrainingReputationNamespace::Build, "trn/build"),
            (
                PylonTrainingReputationNamespace::Checkpoint,
                "trn/checkpoint",
            ),
        ] {
            assert_eq!(namespace.label(), label);
            assert_eq!(
                PylonTrainingReputationNamespace::parse(label),
                Some(namespace)
            );
        }

        for (label, value) in [
            (PylonTrainingReputationLabel::Good, "good"),
            (PylonTrainingReputationLabel::Poor, "poor"),
            (PylonTrainingReputationLabel::Quarantined, "quarantined"),
            (PylonTrainingReputationLabel::Fraud, "fraud"),
            (PylonTrainingReputationLabel::Inconsistent, "inconsistent"),
            (PylonTrainingReputationLabel::Admitted, "admitted"),
            (PylonTrainingReputationLabel::Stale, "stale"),
            (PylonTrainingReputationLabel::Revoked, "revoked"),
            (PylonTrainingReputationLabel::Warning, "warning"),
        ] {
            assert_eq!(label.label(), value);
            assert_eq!(PylonTrainingReputationLabel::parse(value), Some(label));
        }
    }

    #[test]
    fn reputation_projection_tracks_age_and_hard_gate_bits() {
        let projected = reputation_projection_for_label(
            PylonTrainingReputationNamespace::Validator,
            PylonTrainingReputationLabel::Poor,
            1_700_000_000,
            1_700_000_000 + (5 * 86_400),
        )
        .expect("projection should resolve");
        assert_eq!(projected.age_days, 5);
        assert_eq!(
            projected.scheduler_effect,
            PylonTrainingSchedulerEffect::SoftNegative
        );
        assert!(!projected.hard_gate);
        assert_eq!(projected.scheduler_effect.label(), "soft_negative");

        let aged_out = reputation_projection_for_label(
            PylonTrainingReputationNamespace::Checkpoint,
            PylonTrainingReputationLabel::Warning,
            1_700_000_000,
            1_700_000_000 + (31 * 86_400),
        )
        .expect("projection should resolve");
        assert_eq!(aged_out.age_days, 31);
        assert_eq!(
            aged_out.scheduler_effect,
            PylonTrainingSchedulerEffect::Ignored
        );
        assert!(!aged_out.hard_gate);

        let hard_gate = reputation_projection_for_label(
            PylonTrainingReputationNamespace::Build,
            PylonTrainingReputationLabel::Revoked,
            1_700_000_000,
            1_700_000_100,
        )
        .expect("projection should resolve");
        assert!(hard_gate.hard_gate);
        assert_eq!(
            hard_gate.scheduler_effect,
            PylonTrainingSchedulerEffect::HardGate
        );
        assert_eq!(hard_gate.scheduler_effect.label(), "hard_gate");
    }

    #[test]
    fn observability_context_rejects_placeholder_ids() {
        PylonTrainingObservabilityContext {
            network_id: Some("trainnet.alpha".to_string()),
            run_id: Some("run.alpha".to_string()),
            window_id: Some("window.000123".to_string()),
            assignment_id: None,
            challenge_id: None,
            node_pubkey: Some("nodepub".to_string()),
            membership_revision: Some("members.rev7".to_string()),
            manifest_digest: Some("sha256:manifest".to_string()),
        }
        .validate()
        .expect("real identifiers should validate");

        let err = PylonTrainingObservabilityContext {
            challenge_id: Some("placeholder".to_string()),
            ..PylonTrainingObservabilityContext::default()
        }
        .validate()
        .expect_err("placeholder ids should fail");
        assert_eq!(err, "pylon_training_observability_challenge_id_placeholder");
    }

    #[test]
    fn drift_constants_remain_frozen() {
        assert_eq!(
            PYLON_TRAINING_RUN_MANIFEST_V1,
            "openagents.pylon_training_run_manifest.v1"
        );
        assert_eq!(
            PYLON_TRAINING_GCS_CREDENTIAL_SOURCE,
            "google_application_default_credentials"
        );
        assert_eq!(PYLON_TRAINING_HEARTBEAT_INTERVAL_MS, 15_000);
        assert_eq!(PYLON_TRAINING_HEARTBEAT_EXPIRY_MS, 60_000);
        assert_eq!(PYLON_TRAINING_LEASE_DURATION_MS, 600_000);
        assert_eq!(PYLON_TRAINING_LEASE_RENEWAL_THRESHOLD_MS, 180_000);
        assert_eq!(PYLON_TRAINING_WINDOW_MAX_DURATION_MS, 1_800_000);
        assert_eq!(PYLON_TRAINING_SEAL_GRACE_PERIOD_MS, 120_000);
        assert_eq!(PYLON_TRAINING_VALIDATOR_TIMEOUT_MS, 900_000);
        assert_eq!(PYLON_TRAINING_UPLOAD_TIMEOUT_MS, 1_200_000);
        assert_eq!(
            PYLON_TRAINING_OBSERVABILITY_FIELDS,
            [
                "network_id",
                "run_id",
                "window_id",
                "assignment_id",
                "challenge_id",
                "node_pubkey",
                "membership_revision",
                "manifest_digest",
            ]
        );
        assert_eq!(
            PYLON_TRAINING_RETRY_SCHEDULE_MS,
            [5_000, 15_000, 30_000, 60_000, 120_000]
        );
        assert_eq!(PYLON_TRAINING_RETRY_CAP_MS, 300_000);
        assert_eq!(
            PylonTrainingAcceptedUnit::SealedWindow,
            PylonTrainingAcceptedUnit::SealedWindow
        );
        assert_eq!(
            PylonTrainingRefusalCode::UnsupportedTopology.label(),
            "unsupported_topology"
        );
        assert_eq!(
            PylonTrainingRefusalCode::BuildRevoked.owner(),
            PylonTrainingAuthorityOwner::Nexus
        );
    }
}
