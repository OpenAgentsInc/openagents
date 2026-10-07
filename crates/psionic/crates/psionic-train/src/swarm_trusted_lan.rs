use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    first_swarm_open_adapter_receipt_contract, first_swarm_run_contract,
    AdapterContributionSecurityReasonCode, AdapterContributionValidatorDisposition,
    TrainingContributorSuspensionReason, TrainingParticipantDepartureReason,
    SWARM_FIRST_RUN_CLUSTER_NAMESPACE, SWARM_FIRST_RUN_DATASET_REF, SWARM_FIRST_RUN_SCOPE_WINDOW,
    SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH, SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
};

/// Stable schema version for the first swarm trusted-LAN topology contract.
pub const FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_SCHEMA_VERSION: &str =
    "swarm.first_trusted_lan_topology_contract.v1";
/// Stable schema version for the first swarm trusted-LAN failure-drill bundle.
pub const FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_SCHEMA_VERSION: &str =
    "swarm.first_trusted_lan_failure_drills.v1";
/// Stable fixture path for the first swarm trusted-LAN topology contract.
pub const FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/swarm/first_swarm_trusted_lan_topology_contract_v1.json";
/// Stable fixture path for the first swarm trusted-LAN failure-drill bundle.
pub const FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_FIXTURE_PATH: &str =
    "fixtures/swarm/reports/first_swarm_trusted_lan_failure_drills_v1.json";
/// Stable fixture path for the first swarm trusted-LAN workflow plan input.
pub const FIRST_SWARM_LIVE_WORKFLOW_PLAN_FIXTURE_PATH: &str =
    "fixtures/swarm/first_swarm_live_workflow_plan_v1.json";
/// Stable contract identifier for the first swarm trusted-LAN topology.
pub const FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_ID: &str =
    "swarm.first_trusted_lan_topology_contract.v1";
/// Stable bundle identifier for the first swarm trusted-LAN failure drills.
pub const FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_BUNDLE_ID: &str =
    "swarm.first_trusted_lan_failure_drills.v1";
/// Stable launch-script entrypoint for the first swarm trusted-LAN lane.
pub const FIRST_SWARM_TRUSTED_LAN_LAUNCH_SCRIPT_PATH: &str =
    "scripts/first-swarm-launch-trusted-lan.sh";
/// Stable operator bundle checker entrypoint for the first swarm trusted-LAN lane.
pub const FIRST_SWARM_TRUSTED_LAN_CHECK_SCRIPT_PATH: &str =
    "scripts/check-first-swarm-trusted-lan.sh";
/// Stable heartbeat interval for the first swarm trusted-LAN lane.
pub const FIRST_SWARM_TRUSTED_LAN_HEARTBEAT_INTERVAL_MS: u64 = 1_000;
/// Stable stale-worker threshold for the first swarm trusted-LAN lane.
pub const FIRST_SWARM_TRUSTED_LAN_STALE_AFTER_MS: u64 = 5_000;
/// Stable contributor-loss grace window for the first swarm trusted-LAN lane.
pub const FIRST_SWARM_TRUSTED_LAN_CONTRIBUTOR_LOSS_GRACE_MS: u64 = 7_500;
/// Stable maximum worker-skew window for the first swarm trusted-LAN lane.
pub const FIRST_SWARM_TRUSTED_LAN_MAX_WORKER_SKEW_MS: u64 = 15_000;
/// Stable operator repo directory expected on both hosts.
pub const FIRST_SWARM_TRUSTED_LAN_REPO_DIR: &str = "~/code/psionic";
/// Stable run-root template expected on the Mac coordinator.
pub const FIRST_SWARM_TRUSTED_LAN_MAC_RUN_ROOT_TEMPLATE: &str = "~/swarm-runs/${RUN_ID}/mac";
/// Stable run-root template expected on the Linux contributor.
pub const FIRST_SWARM_TRUSTED_LAN_LINUX_RUN_ROOT_TEMPLATE: &str = "~/swarm-runs/${RUN_ID}/linux";
/// Stable cluster bind address expected on the Mac coordinator.
pub const FIRST_SWARM_TRUSTED_LAN_MAC_CLUSTER_ADDR: &str = "swarm-mac-a.local:34100";
/// Stable cluster bind address expected on the Linux contributor.
pub const FIRST_SWARM_TRUSTED_LAN_LINUX_CLUSTER_ADDR: &str = "swarm-linux-4080-a.local:34101";
/// Stable admission-token environment variable for the first swarm trusted-LAN lane.
pub const FIRST_SWARM_TRUSTED_LAN_ADMISSION_TOKEN_ENV: &str = "PSIONIC_SWARM_ADMISSION_TOKEN";

/// Errors surfaced while building or writing the first trusted-LAN swarm artifacts.
#[derive(Debug, Error)]
pub enum FirstSwarmTrustedLanError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error("first swarm trusted-LAN fixture contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Stable actor that executes one launch step in the first swarm trusted-LAN lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanLaunchActor {
    /// The human or local operator host that materializes the bundle.
    Operator,
    /// The Mac MLX coordinator and validator host.
    MacCoordinator,
    /// The Linux RTX 4080 contributor host.
    LinuxContributor,
}

/// Stable phase inside the first swarm trusted-LAN launch sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanLaunchPhase {
    /// Materialize the operator bundle and freeze inputs.
    MaterializeOperatorBundle,
    /// Validate the Mac coordinator node.
    ValidateMacCoordinator,
    /// Validate the Linux contributor node.
    ValidateLinuxContributor,
    /// Freeze the live workflow plan against the trusted-LAN topology.
    FreezeWorkflowPlan,
    /// Freeze failure-drill expectations before any rehearsal or live run.
    FreezeFailureDrills,
}

/// One node frozen into the first swarm trusted-LAN topology contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanNodeContract {
    /// Stable node identifier.
    pub node_id: String,
    /// Stable swarm role identifier.
    pub role_id: String,
    /// Stable host alias used in operator notes.
    pub host_alias: String,
    /// Stable platform label.
    pub platform: String,
    /// Stable execution backend label.
    pub backend_label: String,
    /// Stable logical-device label expected from the bring-up report.
    pub logical_device_label: String,
    /// Stable cluster endpoint.
    pub cluster_addr: String,
    /// Stable repo directory expected on the node.
    pub repo_dir: String,
    /// Stable run-root template expected on the node.
    pub run_root_template: String,
    /// Stable artifact stage root expected on the node.
    pub artifact_stage_root: String,
    /// Whether the node owns validator visibility.
    pub validator_visible: bool,
    /// Whether the node owns aggregation visibility.
    pub aggregation_visible: bool,
    /// Stable bring-up report fixture path bound to the node.
    pub required_bringup_report_fixture_path: String,
    /// Stable bring-up report digest bound to the node.
    pub required_bringup_report_digest: String,
}

/// Frozen artifact-staging roots for the first swarm trusted-LAN lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanArtifactStaging {
    /// Stable dataset reference expected by both nodes.
    pub dataset_ref: String,
    /// Stable dataset stage root.
    pub dataset_stage_root: String,
    /// Stable contribution-upload stage root.
    pub contribution_stage_root: String,
    /// Stable validator-receipt stage root.
    pub validator_stage_root: String,
    /// Stable aggregation-output stage root.
    pub aggregation_stage_root: String,
    /// Stable replay-receipt stage root.
    pub replay_stage_root: String,
    /// Stable local-snapshot publication root.
    pub local_snapshot_root: String,
}

/// Frozen heartbeat and stale-worker policy for the first swarm trusted-LAN lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanHeartbeatPolicy {
    /// Heartbeat interval expected during active windows.
    pub heartbeat_interval_ms: u64,
    /// Stale-worker threshold for validator and coordinator use.
    pub stale_after_ms: u64,
    /// Grace period before a departed contributor is recorded as lost.
    pub contributor_loss_grace_ms: u64,
    /// Maximum skew tolerated between contributors before the operator should replay.
    pub max_worker_skew_ms: u64,
}

/// One typed operator launch step frozen by the first swarm trusted-LAN contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanLaunchStep {
    /// Stable launch phase.
    pub phase: FirstSwarmTrustedLanLaunchPhase,
    /// Stable actor responsible for the phase.
    pub actor: FirstSwarmTrustedLanLaunchActor,
    /// Stable command that realizes the phase.
    pub command: String,
    /// Stable retained artifact path produced by the phase.
    pub retained_artifact_path: String,
    /// Short detail explaining the phase.
    pub detail: String,
}

/// Machine-legible trusted-LAN topology contract for the first swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanTopologyContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Stable first swarm run family id.
    pub run_family_id: String,
    /// Stable first swarm scope window.
    pub scope_window: String,
    /// Stable first swarm run contract digest.
    pub swarm_contract_digest: String,
    /// Stable first swarm receipt contract digest.
    pub receipt_contract_digest: String,
    /// Stable live workflow plan digest bound to this topology.
    pub live_workflow_plan_digest: String,
    /// Stable live workflow membership receipt digest bound to this topology.
    pub live_workflow_membership_receipt_digest: String,
    /// Stable cluster namespace.
    pub cluster_namespace: String,
    /// Stable cluster admission posture.
    pub cluster_admission_posture: String,
    /// Stable admission-token environment variable name.
    pub admission_token_env_var: String,
    /// Stable trusted-network posture.
    pub trusted_network_posture: String,
    /// Stable launch-script entrypoint.
    pub launch_script_path: String,
    /// Stable checker entrypoint.
    pub check_script_path: String,
    /// Stable coordinator node id.
    pub coordinator_node_id: String,
    /// Stable contributor node ids.
    pub contributor_node_ids: Vec<String>,
    /// Frozen per-node topology contract.
    pub nodes: Vec<FirstSwarmTrustedLanNodeContract>,
    /// Frozen artifact-staging roots.
    pub artifact_staging: FirstSwarmTrustedLanArtifactStaging,
    /// Frozen heartbeat policy.
    pub heartbeat_policy: FirstSwarmTrustedLanHeartbeatPolicy,
    /// Exact operator launch sequence for this lane.
    pub launch_sequence: Vec<FirstSwarmTrustedLanLaunchStep>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Explicit drift notes.
    pub drift_notes: Vec<String>,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl FirstSwarmTrustedLanTopologyContract {
    /// Returns the stable digest over the topology contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(
            b"psionic_first_swarm_trusted_lan_topology_contract|",
            &clone,
        )
    }

    /// Validates basic contract invariants.
    pub fn validate(&self) -> Result<(), FirstSwarmTrustedLanError> {
        if self.schema_version != FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_SCHEMA_VERSION {
            return Err(FirstSwarmTrustedLanError::InvalidContract {
                detail: format!(
                    "topology schema_version must stay `{FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_SCHEMA_VERSION}`"
                ),
            });
        }
        if self.nodes.len() != 2 {
            return Err(FirstSwarmTrustedLanError::InvalidContract {
                detail: String::from("trusted-LAN topology must keep exactly two nodes"),
            });
        }
        if self.launch_sequence.len() < 5 {
            return Err(FirstSwarmTrustedLanError::InvalidContract {
                detail: String::from(
                    "trusted-LAN topology must keep the full operator launch sequence",
                ),
            });
        }
        if !self
            .contributor_node_ids
            .iter()
            .all(|node_id| self.nodes.iter().any(|node| &node.node_id == node_id))
        {
            return Err(FirstSwarmTrustedLanError::InvalidContract {
                detail: String::from(
                    "trusted-LAN topology contributor_node_ids must reference known nodes",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(FirstSwarmTrustedLanError::InvalidContract {
                detail: String::from("trusted-LAN topology contract digest did not recompute"),
            });
        }
        Ok(())
    }
}

/// Failure mode frozen by the first swarm trusted-LAN drill bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanFailureDrillKind {
    /// The worker stopped heartbeating before validation.
    StaleWorker,
    /// The uploaded artifact manifest drifted from the planned digest.
    UploadDisagreement,
    /// The worker departed during the active window.
    ContributorLoss,
    /// One worker lagged behind the other enough to trigger operator attention.
    UnevenWorkerSpeed,
}

/// Final disposition frozen for one first swarm trusted-LAN drill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanFailureDisposition {
    /// The contribution must be replayed before the lane may continue.
    ReplayRequired,
    /// The contribution must be quarantined and excluded from aggregation.
    Quarantined,
    /// The contribution is rejected for the current window.
    Rejected,
    /// The operator may wait briefly, then replay if skew stays above threshold.
    WaitThenReplay,
}

/// One machine-legible failure drill bound to the first swarm trusted-LAN lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanFailureDrill {
    /// Stable drill identifier.
    pub drill_id: String,
    /// Failure mode being exercised.
    pub drill_kind: FirstSwarmTrustedLanFailureDrillKind,
    /// Node ids affected by the drill.
    pub affected_node_ids: Vec<String>,
    /// Primary validator disposition expected from the drill.
    pub validator_disposition: AdapterContributionValidatorDisposition,
    /// Optional security reason codes expected from the drill.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security_reason_codes: Vec<AdapterContributionSecurityReasonCode>,
    /// Optional contributor departure reason expected from the drill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departure_reason: Option<TrainingParticipantDepartureReason>,
    /// Optional contributor-suspension reason expected from the drill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suspension_reason: Option<TrainingContributorSuspensionReason>,
    /// Stable contributor role id whose upload or heartbeat is being examined.
    pub contributor_role_id: String,
    /// Stable contribution id bound to the drill.
    pub contribution_id: String,
    /// Stable expected upload manifest digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_upload_manifest_digest: Option<String>,
    /// Stable observed upload manifest digest when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_upload_manifest_digest: Option<String>,
    /// Stale-worker threshold in milliseconds when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_after_ms: Option<u64>,
    /// Observed worker skew when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_worker_skew_ms: Option<u64>,
    /// Final failure disposition.
    pub disposition: FirstSwarmTrustedLanFailureDisposition,
    /// Whether aggregation must refuse promotion for the window.
    pub aggregation_blocked: bool,
    /// Whether the run requires a fresh replay under the same contract.
    pub replay_required: bool,
    /// Stable operator action for the drill.
    pub operator_action: String,
    /// Short detail explaining the drill.
    pub detail: String,
    /// Stable drill digest.
    pub drill_digest: String,
}

impl FirstSwarmTrustedLanFailureDrill {
    /// Returns the stable digest over one failure drill.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.drill_digest.clear();
        stable_digest(b"psionic_first_swarm_trusted_lan_failure_drill|", &clone)
    }
}

/// Machine-legible failure-drill bundle for the first swarm trusted-LAN lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanFailureDrillBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable first swarm run family id.
    pub run_family_id: String,
    /// Stable first swarm run contract digest.
    pub swarm_contract_digest: String,
    /// Stable trusted-LAN topology contract digest.
    pub topology_contract_digest: String,
    /// Stable live workflow plan digest.
    pub live_workflow_plan_digest: String,
    /// Typed failure drills.
    pub drills: Vec<FirstSwarmTrustedLanFailureDrill>,
    /// Short summary of the frozen drill posture.
    pub summary: String,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl FirstSwarmTrustedLanFailureDrillBundle {
    /// Returns the stable digest over the failure-drill bundle.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.bundle_digest.clear();
        stable_digest(b"psionic_first_swarm_trusted_lan_failure_drills|", &clone)
    }

    /// Validates the required drill coverage.
    pub fn validate(&self) -> Result<(), FirstSwarmTrustedLanError> {
        if self.schema_version != FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_SCHEMA_VERSION {
            return Err(FirstSwarmTrustedLanError::InvalidContract {
                detail: format!(
                    "failure-drill schema_version must stay `{FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_SCHEMA_VERSION}`"
                ),
            });
        }
        let required = [
            FirstSwarmTrustedLanFailureDrillKind::StaleWorker,
            FirstSwarmTrustedLanFailureDrillKind::UploadDisagreement,
            FirstSwarmTrustedLanFailureDrillKind::ContributorLoss,
            FirstSwarmTrustedLanFailureDrillKind::UnevenWorkerSpeed,
        ];
        for kind in required {
            if !self.drills.iter().any(|drill| drill.drill_kind == kind) {
                return Err(FirstSwarmTrustedLanError::InvalidContract {
                    detail: format!("failure-drill bundle is missing `{kind:?}`"),
                });
            }
        }
        if self.bundle_digest != self.stable_digest() {
            return Err(FirstSwarmTrustedLanError::InvalidContract {
                detail: String::from("failure-drill bundle digest did not recompute"),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedFirstSwarmLiveWorkflowPlan {
    run_family_id: String,
    swarm_contract_digest: String,
    membership_receipt: RetainedMembershipReceipt,
    contributor_assignments: Vec<RetainedContributorAssignment>,
    plan_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedMembershipReceipt {
    receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedContributorAssignment {
    role_id: String,
    contributor_node_id: String,
    expected_upload_manifest_digest: String,
    contribution_id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedBringupReport {
    contract_digest: String,
    report_digest: String,
}

/// Returns the canonical trusted-LAN topology contract for the first swarm lane.
pub fn first_swarm_trusted_lan_topology_contract(
) -> Result<FirstSwarmTrustedLanTopologyContract, FirstSwarmTrustedLanError> {
    let swarm_contract = first_swarm_run_contract();
    let receipt_contract = first_swarm_open_adapter_receipt_contract();
    let plan: RetainedFirstSwarmLiveWorkflowPlan =
        load_repo_fixture(FIRST_SWARM_LIVE_WORKFLOW_PLAN_FIXTURE_PATH)?;
    let mac_bringup: RetainedBringupReport = load_repo_fixture(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH)?;
    let linux_bringup: RetainedBringupReport =
        load_repo_fixture(SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH)?;
    if plan.run_family_id != swarm_contract.run_family_id {
        return Err(FirstSwarmTrustedLanError::InvalidContract {
            detail: String::from(
                "retained live workflow plan drifted from the first swarm run family",
            ),
        });
    }
    if plan.swarm_contract_digest != swarm_contract.contract_digest {
        return Err(FirstSwarmTrustedLanError::InvalidContract {
            detail: String::from(
                "retained live workflow plan drifted from the first swarm contract digest",
            ),
        });
    }
    if mac_bringup.contract_digest != swarm_contract.contract_digest
        || linux_bringup.contract_digest != swarm_contract.contract_digest
    {
        return Err(FirstSwarmTrustedLanError::InvalidContract {
            detail: String::from("retained bring-up reports drifted from the first swarm contract"),
        });
    }

    let nodes = vec![
        FirstSwarmTrustedLanNodeContract {
            node_id: String::from("swarm-mac-a"),
            role_id: String::from("swarm.mac.mlx.coordinator_validator_contributor"),
            host_alias: String::from("swarm-mac-a.local"),
            platform: String::from("macos_apple_silicon"),
            backend_label: String::from("open_adapter_backend.mlx.metal.gpt_oss_lm_head"),
            logical_device_label: String::from("metal:0"),
            cluster_addr: String::from(FIRST_SWARM_TRUSTED_LAN_MAC_CLUSTER_ADDR),
            repo_dir: String::from(FIRST_SWARM_TRUSTED_LAN_REPO_DIR),
            run_root_template: String::from(FIRST_SWARM_TRUSTED_LAN_MAC_RUN_ROOT_TEMPLATE),
            artifact_stage_root: String::from("${RUN_ROOT}/artifacts"),
            validator_visible: true,
            aggregation_visible: true,
            required_bringup_report_fixture_path: String::from(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH),
            required_bringup_report_digest: mac_bringup.report_digest,
        },
        FirstSwarmTrustedLanNodeContract {
            node_id: String::from("swarm-linux-4080-a"),
            role_id: String::from("swarm.linux.cuda.rtx4080.contributor"),
            host_alias: String::from("swarm-linux-4080-a.local"),
            platform: String::from("linux_nvidia_rtx_4080"),
            backend_label: String::from("open_adapter_backend.cuda.gpt_oss_lm_head"),
            logical_device_label: String::from("cuda:0"),
            cluster_addr: String::from(FIRST_SWARM_TRUSTED_LAN_LINUX_CLUSTER_ADDR),
            repo_dir: String::from(FIRST_SWARM_TRUSTED_LAN_REPO_DIR),
            run_root_template: String::from(FIRST_SWARM_TRUSTED_LAN_LINUX_RUN_ROOT_TEMPLATE),
            artifact_stage_root: String::from("${RUN_ROOT}/artifacts"),
            validator_visible: false,
            aggregation_visible: false,
            required_bringup_report_fixture_path: String::from(
                SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH,
            ),
            required_bringup_report_digest: linux_bringup.report_digest,
        },
    ];
    let artifact_staging = FirstSwarmTrustedLanArtifactStaging {
        dataset_ref: String::from(SWARM_FIRST_RUN_DATASET_REF),
        dataset_stage_root: String::from("${RUN_ROOT}/dataset"),
        contribution_stage_root: String::from("${RUN_ROOT}/artifacts/contributions"),
        validator_stage_root: String::from("${RUN_ROOT}/artifacts/validator"),
        aggregation_stage_root: String::from("${RUN_ROOT}/artifacts/aggregation"),
        replay_stage_root: String::from("${RUN_ROOT}/artifacts/replay"),
        local_snapshot_root: String::from("${RUN_ROOT}/local_publish"),
    };
    let heartbeat_policy = FirstSwarmTrustedLanHeartbeatPolicy {
        heartbeat_interval_ms: FIRST_SWARM_TRUSTED_LAN_HEARTBEAT_INTERVAL_MS,
        stale_after_ms: FIRST_SWARM_TRUSTED_LAN_STALE_AFTER_MS,
        contributor_loss_grace_ms: FIRST_SWARM_TRUSTED_LAN_CONTRIBUTOR_LOSS_GRACE_MS,
        max_worker_skew_ms: FIRST_SWARM_TRUSTED_LAN_MAX_WORKER_SKEW_MS,
    };
    let launch_sequence = vec![
        FirstSwarmTrustedLanLaunchStep {
            phase: FirstSwarmTrustedLanLaunchPhase::MaterializeOperatorBundle,
            actor: FirstSwarmTrustedLanLaunchActor::Operator,
            command: String::from(
                "scripts/first-swarm-launch-trusted-lan.sh --run-id ${RUN_ID} --bundle-dir ${BUNDLE_DIR}",
            ),
            retained_artifact_path: String::from("${BUNDLE_DIR}/first_swarm_trusted_lan_launch_manifest.json"),
            detail: String::from(
                "Materialize one operator bundle that freezes the topology contract, failure drills, retained bring-up inputs, and the current first-swarm workflow plan before either host starts contributing.",
            ),
        },
        FirstSwarmTrustedLanLaunchStep {
            phase: FirstSwarmTrustedLanLaunchPhase::ValidateMacCoordinator,
            actor: FirstSwarmTrustedLanLaunchActor::MacCoordinator,
            command: String::from(
                "scripts/check-swarm-mac-mlx-bringup.sh --report ${MAC_RUN_ROOT}/reports/swarm_mac_mlx_bringup_v1.json",
            ),
            retained_artifact_path: String::from("${MAC_RUN_ROOT}/reports/swarm_mac_mlx_bringup_v1.json"),
            detail: String::from(
                "The Mac host must prove the MLX Metal same-node gate and the shared contributor-receipt contract before the cluster lane may proceed.",
            ),
        },
        FirstSwarmTrustedLanLaunchStep {
            phase: FirstSwarmTrustedLanLaunchPhase::ValidateLinuxContributor,
            actor: FirstSwarmTrustedLanLaunchActor::LinuxContributor,
            command: String::from(
                "scripts/check-swarm-linux-4080-bringup.sh --report ${LINUX_RUN_ROOT}/reports/swarm_linux_rtx4080_bringup_v1.json",
            ),
            retained_artifact_path: String::from("${LINUX_RUN_ROOT}/reports/swarm_linux_rtx4080_bringup_v1.json"),
            detail: String::from(
                "The Linux host must prove the retained RTX 4080 contract plus the same-node CUDA open-adapter parity harness before the cluster lane may proceed.",
            ),
        },
        FirstSwarmTrustedLanLaunchStep {
            phase: FirstSwarmTrustedLanLaunchPhase::FreezeWorkflowPlan,
            actor: FirstSwarmTrustedLanLaunchActor::MacCoordinator,
            command: String::from(
                "cargo run -q -p psionic-mlx-workflows --bin first_swarm_live_workflow_plan -- ${BUNDLE_DIR}/first_swarm_live_workflow_plan_v1.json",
            ),
            retained_artifact_path: String::from("${BUNDLE_DIR}/first_swarm_live_workflow_plan_v1.json"),
            detail: String::from(
                "The coordinator freezes contributor selection, dataset slices, expected upload digests, and local snapshot posture against the exact trusted-LAN node set.",
            ),
        },
        FirstSwarmTrustedLanLaunchStep {
            phase: FirstSwarmTrustedLanLaunchPhase::FreezeFailureDrills,
            actor: FirstSwarmTrustedLanLaunchActor::MacCoordinator,
            command: String::from(
                "cargo run -q -p psionic-train --bin first_swarm_trusted_lan_failure_drills -- ${BUNDLE_DIR}/reports/first_swarm_trusted_lan_failure_drills_v1.json",
            ),
            retained_artifact_path: String::from("${BUNDLE_DIR}/reports/first_swarm_trusted_lan_failure_drills_v1.json"),
            detail: String::from(
                "The coordinator freezes stale-worker, upload-disagreement, contributor-loss, and skew handling before any rehearsal or live run claims are made.",
            ),
        },
    ];
    let mut contract = FirstSwarmTrustedLanTopologyContract {
        schema_version: String::from(FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_SCHEMA_VERSION),
        contract_id: String::from(FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_ID),
        run_family_id: swarm_contract.run_family_id.clone(),
        scope_window: String::from(SWARM_FIRST_RUN_SCOPE_WINDOW),
        swarm_contract_digest: swarm_contract.contract_digest,
        receipt_contract_digest: receipt_contract.contract_digest,
        live_workflow_plan_digest: plan.plan_digest,
        live_workflow_membership_receipt_digest: plan.membership_receipt.receipt_digest,
        cluster_namespace: String::from(SWARM_FIRST_RUN_CLUSTER_NAMESPACE),
        cluster_admission_posture: swarm_contract.cluster_admission_posture,
        admission_token_env_var: String::from(FIRST_SWARM_TRUSTED_LAN_ADMISSION_TOKEN_ENV),
        trusted_network_posture: String::from(
            "trusted_lan_only.no_internet_discovery.no_cross_subnet_overclaim",
        ),
        launch_script_path: String::from(FIRST_SWARM_TRUSTED_LAN_LAUNCH_SCRIPT_PATH),
        check_script_path: String::from(FIRST_SWARM_TRUSTED_LAN_CHECK_SCRIPT_PATH),
        coordinator_node_id: String::from("swarm-mac-a"),
        contributor_node_ids: vec![
            String::from("swarm-mac-a"),
            String::from("swarm-linux-4080-a"),
        ],
        nodes,
        artifact_staging,
        heartbeat_policy,
        launch_sequence,
        claim_boundary: String::from(
            "This contract freezes one trusted-LAN two-node swarm topology for the first mixed-hardware open-adapter lane: one Mac MLX Metal coordinator, validator, and aggregator plus one Linux RTX 4080 CUDA contributor. It proves exact node identity, artifact staging, heartbeat and stale-worker posture, and launch sequencing. It does not claim wider-network discovery, elastic world size, or a finished live two-node trainer by itself.",
        ),
        drift_notes: vec![
            String::from(
                "The topology binds the current retained first-swarm workflow plan instead of inventing a second planner surface for the exact same lane.",
            ),
            String::from(
                "The Linux node still depends on the retained RTX 4080 bring-up report and bounded same-node parity harness rather than a new remote-probe subsystem.",
            ),
        ],
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

/// Writes the canonical trusted-LAN topology contract to one JSON path.
pub fn write_first_swarm_trusted_lan_topology_contract(
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmTrustedLanTopologyContract, FirstSwarmTrustedLanError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| FirstSwarmTrustedLanError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let contract = first_swarm_trusted_lan_topology_contract()?;
    let encoded = serde_json::to_string_pretty(&contract)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmTrustedLanError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(contract)
}

/// Returns the canonical trusted-LAN failure-drill bundle for the first swarm lane.
pub fn first_swarm_trusted_lan_failure_drill_bundle(
) -> Result<FirstSwarmTrustedLanFailureDrillBundle, FirstSwarmTrustedLanError> {
    let topology = first_swarm_trusted_lan_topology_contract()?;
    let plan: RetainedFirstSwarmLiveWorkflowPlan =
        load_repo_fixture(FIRST_SWARM_LIVE_WORKFLOW_PLAN_FIXTURE_PATH)?;

    let mac_assignment = plan
        .contributor_assignments
        .iter()
        .find(|assignment| {
            assignment.role_id == "swarm.mac.mlx.coordinator_validator_contributor"
                && assignment.contributor_node_id == "swarm-mac-a"
        })
        .ok_or_else(|| FirstSwarmTrustedLanError::InvalidContract {
            detail: String::from("live workflow plan is missing the Mac contributor assignment"),
        })?;
    let linux_assignment = plan
        .contributor_assignments
        .iter()
        .find(|assignment| {
            assignment.role_id == "swarm.linux.cuda.rtx4080.contributor"
                && assignment.contributor_node_id == "swarm-linux-4080-a"
        })
        .ok_or_else(|| FirstSwarmTrustedLanError::InvalidContract {
            detail: String::from("live workflow plan is missing the Linux contributor assignment"),
        })?;
    let drills = vec![
        build_failure_drill(
            FirstSwarmTrustedLanFailureDrillKind::StaleWorker,
            vec![String::from("swarm-linux-4080-a")],
            AdapterContributionValidatorDisposition::ReplayRequired,
            vec![AdapterContributionSecurityReasonCode::StaleSession],
            Some(TrainingParticipantDepartureReason::TimedOut),
            Some(TrainingContributorSuspensionReason::CapabilityPrerequisiteMissing),
            linux_assignment.role_id.clone(),
            linux_assignment.contribution_id.clone(),
            Some(linux_assignment.expected_upload_manifest_digest.clone()),
            None,
            Some(FIRST_SWARM_TRUSTED_LAN_STALE_AFTER_MS),
            None,
            FirstSwarmTrustedLanFailureDisposition::ReplayRequired,
            true,
            true,
            String::from(
                "Mark the Linux contributor stale, refuse aggregation for the window, and replay the same dataset slice under a fresh contributor-set revision.",
            ),
            String::from(
                "If the Linux worker stops heartbeating for more than five seconds during the active window, the coordinator records contributor loss, the validator keeps replay-required truth explicit, and the lane does not aggregate a one-node partial result.",
            ),
        ),
        build_failure_drill(
            FirstSwarmTrustedLanFailureDrillKind::UploadDisagreement,
            vec![String::from("swarm-linux-4080-a")],
            AdapterContributionValidatorDisposition::Rejected,
            vec![AdapterContributionSecurityReasonCode::ManifestDigestMismatch],
            None,
            Some(TrainingContributorSuspensionReason::DuplicateContribution),
            linux_assignment.role_id.clone(),
            linux_assignment.contribution_id.clone(),
            Some(linux_assignment.expected_upload_manifest_digest.clone()),
            Some(mismatched_manifest_digest(
                linux_assignment.expected_upload_manifest_digest.as_str(),
                "upload_disagreement",
            )),
            None,
            None,
            FirstSwarmTrustedLanFailureDisposition::Rejected,
            true,
            false,
            String::from(
                "Quarantine the mismatched upload, keep the validator reason explicit, and refuse promotion until the contributor replays with the planned manifest digest.",
            ),
            String::from(
                "If the uploaded contribution manifest does not match the workflow-plan digest for the Linux node, the validator rejects the contribution and the lane stays no-promotion instead of silently accepting drifted artifacts.",
            ),
        ),
        build_failure_drill(
            FirstSwarmTrustedLanFailureDrillKind::ContributorLoss,
            vec![String::from("swarm-linux-4080-a")],
            AdapterContributionValidatorDisposition::ReplayRequired,
            Vec::new(),
            Some(TrainingParticipantDepartureReason::Crashed),
            Some(TrainingContributorSuspensionReason::OperatorHold),
            linux_assignment.role_id.clone(),
            linux_assignment.contribution_id.clone(),
            Some(linux_assignment.expected_upload_manifest_digest.clone()),
            None,
            None,
            None,
            FirstSwarmTrustedLanFailureDisposition::ReplayRequired,
            true,
            true,
            String::from(
                "Record the contributor as departed, preserve replay-required validator truth, and refuse aggregation until the lane is replayed with both contributor roles present.",
            ),
            String::from(
                "If the Linux contributor disappears during the active window, the coordinator keeps the departure reason explicit and the window seals without promotion because the first lane requires both contributor roles.",
            ),
        ),
        build_failure_drill(
            FirstSwarmTrustedLanFailureDrillKind::UnevenWorkerSpeed,
            vec![String::from("swarm-mac-a"), String::from("swarm-linux-4080-a")],
            AdapterContributionValidatorDisposition::Quarantined,
            Vec::new(),
            None,
            Some(TrainingContributorSuspensionReason::ReliabilityPenalty),
            mac_assignment.role_id.clone(),
            mac_assignment.contribution_id.clone(),
            Some(mac_assignment.expected_upload_manifest_digest.clone()),
            None,
            None,
            Some(FIRST_SWARM_TRUSTED_LAN_MAX_WORKER_SKEW_MS + 3_000),
            FirstSwarmTrustedLanFailureDisposition::WaitThenReplay,
            true,
            true,
            String::from(
                "If contributor skew exceeds the fifteen-second bound, wait briefly, then replay the slower slice instead of hiding idle time or late uploads behind aggregate success language.",
            ),
            String::from(
                "The first swarm lane treats sustained skew above the frozen bound as a real operator-visible bottleneck. The coordinator does not silently stretch the window forever or call the result healthy when one node idles waiting for the other.",
            ),
        ),
    ];
    let mut bundle = FirstSwarmTrustedLanFailureDrillBundle {
        schema_version: String::from(FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_SCHEMA_VERSION),
        bundle_id: String::from(FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_BUNDLE_ID),
        run_family_id: topology.run_family_id.clone(),
        swarm_contract_digest: topology.swarm_contract_digest.clone(),
        topology_contract_digest: topology.contract_digest.clone(),
        live_workflow_plan_digest: topology.live_workflow_plan_digest.clone(),
        drills,
        summary: String::from(
            "The first swarm trusted-LAN drill bundle keeps four exact failure classes explicit: stale-worker timeout, upload-manifest disagreement, contributor loss, and sustained worker skew. Each drill names the affected node, the exact validator posture, whether replay is required, and the operator action that preserves no-promotion truth when the lane stops being comparable.",
        ),
        claim_boundary: String::from(
            "This bundle freezes refusal and replay handling for the first trusted-LAN two-node swarm lane. It is a machine-legible operator drill bundle, not a live-run evidence bundle.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = bundle.stable_digest();
    bundle.validate()?;
    Ok(bundle)
}

/// Writes the canonical trusted-LAN failure-drill bundle to one JSON path.
pub fn write_first_swarm_trusted_lan_failure_drill_bundle(
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmTrustedLanFailureDrillBundle, FirstSwarmTrustedLanError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| FirstSwarmTrustedLanError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bundle = first_swarm_trusted_lan_failure_drill_bundle()?;
    let encoded = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmTrustedLanError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn build_failure_drill(
    drill_kind: FirstSwarmTrustedLanFailureDrillKind,
    affected_node_ids: Vec<String>,
    validator_disposition: AdapterContributionValidatorDisposition,
    security_reason_codes: Vec<AdapterContributionSecurityReasonCode>,
    departure_reason: Option<TrainingParticipantDepartureReason>,
    suspension_reason: Option<TrainingContributorSuspensionReason>,
    contributor_role_id: String,
    contribution_id: String,
    expected_upload_manifest_digest: Option<String>,
    observed_upload_manifest_digest: Option<String>,
    stale_after_ms: Option<u64>,
    observed_worker_skew_ms: Option<u64>,
    disposition: FirstSwarmTrustedLanFailureDisposition,
    aggregation_blocked: bool,
    replay_required: bool,
    operator_action: String,
    detail: String,
) -> FirstSwarmTrustedLanFailureDrill {
    let mut drill = FirstSwarmTrustedLanFailureDrill {
        drill_id: format!(
            "first-swarm-trusted-lan:{}",
            first_swarm_failure_drill_kind_label(drill_kind)
        ),
        drill_kind,
        affected_node_ids,
        validator_disposition,
        security_reason_codes,
        departure_reason,
        suspension_reason,
        contributor_role_id,
        contribution_id,
        expected_upload_manifest_digest,
        observed_upload_manifest_digest,
        stale_after_ms,
        observed_worker_skew_ms,
        disposition,
        aggregation_blocked,
        replay_required,
        operator_action,
        detail,
        drill_digest: String::new(),
    };
    drill.drill_digest = drill.stable_digest();
    drill
}

fn first_swarm_failure_drill_kind_label(
    kind: FirstSwarmTrustedLanFailureDrillKind,
) -> &'static str {
    match kind {
        FirstSwarmTrustedLanFailureDrillKind::StaleWorker => "stale_worker",
        FirstSwarmTrustedLanFailureDrillKind::UploadDisagreement => "upload_disagreement",
        FirstSwarmTrustedLanFailureDrillKind::ContributorLoss => "contributor_loss",
        FirstSwarmTrustedLanFailureDrillKind::UnevenWorkerSpeed => "uneven_worker_speed",
    }
}

fn mismatched_manifest_digest(expected: &str, label: &str) -> String {
    stable_digest(
        b"psionic_first_swarm_trusted_lan_mismatched_manifest|",
        &format!("{label}:{expected}"),
    )
}

fn load_repo_fixture<T>(relative_path: &str) -> Result<T, FirstSwarmTrustedLanError>
where
    T: DeserializeOwned,
{
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| FirstSwarmTrustedLanError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| FirstSwarmTrustedLanError::Deserialize {
        path: path.display().to_string(),
        error,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable digest serialization"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{
        first_swarm_trusted_lan_failure_drill_bundle, first_swarm_trusted_lan_topology_contract,
        write_first_swarm_trusted_lan_failure_drill_bundle,
        write_first_swarm_trusted_lan_topology_contract, FirstSwarmTrustedLanFailureDisposition,
        FirstSwarmTrustedLanFailureDrillBundle, FirstSwarmTrustedLanFailureDrillKind,
        FirstSwarmTrustedLanTopologyContract, FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_FIXTURE_PATH,
        FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_FIXTURE_PATH,
    };

    fn load_fixture<T>(relative_path: &str) -> T
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative_path);
        serde_json::from_slice(&fs::read(path).expect("fixture bytes")).expect("fixture decode")
    }

    #[test]
    fn first_swarm_trusted_lan_topology_stays_aligned_with_swarm_contract() {
        let contract = first_swarm_trusted_lan_topology_contract()
            .expect("trusted-lan topology contract should build");
        assert_eq!(
            contract.cluster_namespace,
            "cluster.swarm.local.trusted_lan"
        );
        assert_eq!(contract.coordinator_node_id, "swarm-mac-a");
        assert_eq!(contract.contributor_node_ids.len(), 2);
        assert!(contract.launch_sequence.iter().any(|step| step
            .retained_artifact_path
            .contains("first_swarm_live_workflow_plan_v1.json")));
    }

    #[test]
    fn first_swarm_failure_drill_bundle_covers_required_cases() {
        let bundle = first_swarm_trusted_lan_failure_drill_bundle()
            .expect("trusted-lan failure drills should build");
        assert_eq!(bundle.drills.len(), 4);
        assert!(bundle.drills.iter().any(|drill| {
            drill.drill_kind == FirstSwarmTrustedLanFailureDrillKind::StaleWorker
                && drill.disposition == FirstSwarmTrustedLanFailureDisposition::ReplayRequired
        }));
        assert!(bundle.drills.iter().any(|drill| {
            drill.drill_kind == FirstSwarmTrustedLanFailureDrillKind::UploadDisagreement
                && drill.disposition == FirstSwarmTrustedLanFailureDisposition::Rejected
        }));
    }

    #[test]
    fn retained_first_swarm_trusted_lan_topology_fixture_matches_builder() {
        let retained: FirstSwarmTrustedLanTopologyContract =
            load_fixture(FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_FIXTURE_PATH);
        let rebuilt = write_first_swarm_trusted_lan_topology_contract(
            tempfile::tempdir()
                .expect("tempdir")
                .path()
                .join("first_swarm_trusted_lan_topology_contract.json"),
        )
        .expect("rebuilt topology contract");
        assert_eq!(retained, rebuilt);
    }

    #[test]
    fn retained_first_swarm_trusted_lan_failure_drills_fixture_matches_builder() {
        let retained: FirstSwarmTrustedLanFailureDrillBundle =
            load_fixture(FIRST_SWARM_TRUSTED_LAN_FAILURE_DRILL_FIXTURE_PATH);
        let rebuilt = write_first_swarm_trusted_lan_failure_drill_bundle(
            tempfile::tempdir()
                .expect("tempdir")
                .path()
                .join("first_swarm_trusted_lan_failure_drills.json"),
        )
        .expect("rebuilt failure drills");
        assert_eq!(retained, rebuilt);
    }
}
