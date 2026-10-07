use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FirstSwarmTrustedLanGoNoGoRecommendation, FirstSwarmTrustedLanRehearsalReport,
    FirstSwarmTrustedLanTopologyContract, FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_FIXTURE_PATH,
};

/// Stable schema version for the first swarm trusted-LAN evidence bundle.
pub const FIRST_SWARM_TRUSTED_LAN_EVIDENCE_BUNDLE_SCHEMA_VERSION: &str =
    "swarm.first_trusted_lan_evidence_bundle.v1";
/// Stable fixture path for the first swarm trusted-LAN evidence bundle.
pub const FIRST_SWARM_TRUSTED_LAN_EVIDENCE_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/swarm/reports/first_swarm_trusted_lan_evidence_bundle_v1.json";

/// Errors surfaced while building or writing the first swarm evidence bundle.
#[derive(Debug, Error)]
pub enum FirstSwarmTrustedLanEvidenceBundleError {
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
    #[error("failed to execute the first swarm trusted-LAN launcher: {detail}")]
    LauncherFailure { detail: String },
    #[error("first swarm live attempt cannot proceed while the rehearsal recommendation is `{recommendation:?}`")]
    LiveAttemptBlocked {
        recommendation: FirstSwarmTrustedLanGoNoGoRecommendation,
    },
}

/// Final disposition of the first swarm trusted-LAN live attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanLiveAttemptDisposition {
    /// The exact lane was refused before remote contribution execution.
    Refused,
    /// The exact lane completed a live attempt.
    Completed,
}

/// Upload posture for one contributor inside the evidence bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanUploadPosture {
    /// Only the planned upload manifest exists.
    PlannedOnly,
    /// A real upload manifest was retained.
    Uploaded,
}

/// Validator posture for one contributor inside the evidence bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanValidatorPosture {
    /// No validator execution occurred.
    NotExecuted,
    /// The contributor was accepted.
    Accepted,
    /// The contributor was quarantined.
    Quarantined,
    /// The contributor was rejected.
    Rejected,
    /// The contributor requires replay.
    ReplayRequired,
}

/// Aggregation posture for one contributor inside the evidence bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanAggregationPosture {
    /// Aggregation never ran.
    NotExecuted,
    /// The contributor was ineligible.
    Ineligible,
    /// The contributor was eligible.
    Eligible,
}

/// Replay posture for one contributor inside the evidence bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanReplayPosture {
    /// No replay receipt exists because execution never started.
    NotExecuted,
    /// The lane explicitly requires replay before another live attempt.
    ReplayRequired,
    /// Replay completed successfully.
    Completed,
}

/// Stage disposition for one evidence-bundle stage summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanStageDisposition {
    /// Stage completed.
    Completed,
    /// Stage was refused.
    Refused,
    /// Stage was skipped.
    Skipped,
}

/// Promotion outcome for the first swarm trusted-LAN lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanPromotionDisposition {
    /// No promotion or local snapshot publication occurred.
    NoPromotion,
    /// A local snapshot was promoted.
    Promoted,
}

/// Per-contributor truth retained in the evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanContributorEvidence {
    /// Stable role identifier.
    pub role_id: String,
    /// Stable node id.
    pub node_id: String,
    /// Stable backend label.
    pub backend_label: String,
    /// Stable membership receipt digest.
    pub membership_receipt_digest: String,
    /// Stable contribution id.
    pub contribution_id: String,
    /// Stable expected upload reference from the workflow plan.
    pub expected_upload_reference: String,
    /// Stable expected upload manifest digest from the workflow plan.
    pub expected_upload_manifest_digest: String,
    /// Upload posture retained in this bundle.
    pub upload_posture: FirstSwarmTrustedLanUploadPosture,
    /// Validator posture retained in this bundle.
    pub validator_posture: FirstSwarmTrustedLanValidatorPosture,
    /// Aggregation posture retained in this bundle.
    pub aggregation_posture: FirstSwarmTrustedLanAggregationPosture,
    /// Replay posture retained in this bundle.
    pub replay_posture: FirstSwarmTrustedLanReplayPosture,
    /// Short detail for the contributor row.
    pub detail: String,
}

/// Stage summary retained in the evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanStageSummary {
    /// Stable stage identifier.
    pub stage_id: String,
    /// Final stage disposition.
    pub disposition: FirstSwarmTrustedLanStageDisposition,
    /// Wallclock attributed to the stage.
    pub wallclock_ms: u64,
    /// Short stage detail.
    pub detail: String,
    /// Stable stage digest.
    pub stage_digest: String,
}

impl FirstSwarmTrustedLanStageSummary {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.stage_digest.clear();
        stable_digest(b"psionic_first_swarm_trusted_lan_stage_summary|", &clone)
    }
}

/// Promotion or no-promotion truth for the evidence bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanPromotionOutcome {
    /// Promotion disposition.
    pub disposition: FirstSwarmTrustedLanPromotionDisposition,
    /// Explicit reason for the outcome.
    pub reason: String,
    /// Optional local snapshot path when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_snapshot_path: Option<String>,
}

/// Full retained evidence bundle for the first swarm trusted-LAN live attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanEvidenceBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Stable run family id.
    pub run_family_id: String,
    /// Stable first swarm run contract digest.
    pub swarm_contract_digest: String,
    /// Stable trusted-LAN topology contract digest.
    pub topology_contract_digest: String,
    /// Stable rehearsal report digest.
    pub rehearsal_report_digest: String,
    /// Stable failure-drill bundle digest.
    pub failure_drills_digest: String,
    /// Stable workflow-plan digest.
    pub workflow_plan_digest: String,
    /// Stable membership receipt digest.
    pub membership_receipt_digest: String,
    /// Stable launch-manifest digest.
    pub launch_manifest_digest: String,
    /// Stable launch-receipt digest.
    pub launch_receipt_digest: String,
    /// Launch status observed from the exact launcher.
    pub launch_status: String,
    /// Final live-attempt disposition.
    pub live_attempt_disposition: FirstSwarmTrustedLanLiveAttemptDisposition,
    /// Explicit reason for the live-attempt outcome.
    pub live_attempt_reason: String,
    /// Per-contributor retained evidence rows.
    pub contributors: Vec<FirstSwarmTrustedLanContributorEvidence>,
    /// Stage summaries for the attempt.
    pub stages: Vec<FirstSwarmTrustedLanStageSummary>,
    /// Promotion or no-promotion truth.
    pub promotion_outcome: FirstSwarmTrustedLanPromotionOutcome,
    /// Explicit drift notes.
    pub drift_notes: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl FirstSwarmTrustedLanEvidenceBundle {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.bundle_digest.clear();
        stable_digest(b"psionic_first_swarm_trusted_lan_evidence_bundle|", &clone)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct FirstSwarmLaunchReceipt {
    launch_status: String,
    manifest_digest: String,
    phase_results: Vec<FirstSwarmLaunchPhaseResult>,
}

#[derive(Clone, Debug, Deserialize)]
struct FirstSwarmLaunchPhaseResult {
    status: String,
    started_at_ms: u64,
    finished_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct FirstSwarmLiveWorkflowPlan {
    plan_digest: String,
    membership_receipt: FirstSwarmMembershipReceipt,
    contributor_assignments: Vec<FirstSwarmContributorAssignment>,
}

#[derive(Clone, Debug, Deserialize)]
struct FirstSwarmMembershipReceipt {
    receipt_digest: String,
}

#[derive(Clone, Debug, Deserialize)]
struct FirstSwarmContributorAssignment {
    role_id: String,
    contributor_node_id: String,
    matched_backend_label: String,
    contribution_id: String,
    expected_upload_reference: String,
    expected_upload_manifest_digest: String,
}

/// Builds the retained evidence bundle for the first swarm trusted-LAN live attempt.
pub fn build_first_swarm_trusted_lan_evidence_bundle(
) -> Result<FirstSwarmTrustedLanEvidenceBundle, FirstSwarmTrustedLanEvidenceBundleError> {
    let temp_bundle_dir = temp_bundle_dir();
    fs::create_dir_all(&temp_bundle_dir).map_err(|error| {
        FirstSwarmTrustedLanEvidenceBundleError::CreateDir {
            path: temp_bundle_dir.display().to_string(),
            error,
        }
    })?;

    let launcher_output = Command::new("bash")
        .arg(repo_root().join("scripts/first-swarm-launch-trusted-lan.sh"))
        .arg("--run-id")
        .arg("first-swarm-trusted-lan-live-attempt")
        .arg("--bundle-dir")
        .arg(&temp_bundle_dir)
        .arg("--manifest-only")
        .current_dir(repo_root())
        .output()
        .map_err(
            |error| FirstSwarmTrustedLanEvidenceBundleError::LauncherFailure {
                detail: error.to_string(),
            },
        )?;
    if !launcher_output.status.success() {
        return Err(FirstSwarmTrustedLanEvidenceBundleError::LauncherFailure {
            detail: String::from_utf8_lossy(&launcher_output.stderr)
                .trim()
                .to_string(),
        });
    }

    let manifest_path = temp_bundle_dir.join("first_swarm_trusted_lan_launch_manifest.json");
    let receipt_path = temp_bundle_dir.join("first_swarm_trusted_lan_launch_receipt.json");
    let topology_path = temp_bundle_dir.join("first_swarm_trusted_lan_topology_contract_v1.json");
    let workflow_plan_path = temp_bundle_dir.join("first_swarm_live_workflow_plan_v1.json");

    let topology: FirstSwarmTrustedLanTopologyContract = load_json(&topology_path)?;
    let launch_receipt: FirstSwarmLaunchReceipt = load_json(&receipt_path)?;
    let workflow_plan: FirstSwarmLiveWorkflowPlan = load_json(&workflow_plan_path)?;
    let rehearsal_report: FirstSwarmTrustedLanRehearsalReport =
        load_repo_fixture(FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_FIXTURE_PATH)?;

    if rehearsal_report.recommendation != FirstSwarmTrustedLanGoNoGoRecommendation::NoGo {
        return Err(
            FirstSwarmTrustedLanEvidenceBundleError::LiveAttemptBlocked {
                recommendation: rehearsal_report.recommendation,
            },
        );
    }

    let launch_manifest_digest = file_sha256(&manifest_path)?;
    let launch_receipt_digest = file_sha256(&receipt_path)?;
    let bundle_materialization_ms = launch_receipt
        .phase_results
        .iter()
        .filter(|phase| phase.status == "completed")
        .map(|phase| phase.finished_at_ms.saturating_sub(phase.started_at_ms))
        .sum::<u64>();

    let gate_started = Instant::now();
    let live_attempt_reason = String::from(
        "The live attempt is refused because the committed rehearsal report still ends `no_go`: no live two-node contributor execution receipt exists for the exact trusted-LAN lane, upload/validator/aggregation timing are still simulated, and no promotion or no-promotion receipt has yet been earned from a real contribution set.",
    );
    let gate_wallclock_ms = gate_started.elapsed().as_millis() as u64;

    let contributors = workflow_plan
        .contributor_assignments
        .iter()
        .map(|assignment| FirstSwarmTrustedLanContributorEvidence {
            role_id: assignment.role_id.clone(),
            node_id: assignment.contributor_node_id.clone(),
            backend_label: assignment.matched_backend_label.clone(),
            membership_receipt_digest: workflow_plan.membership_receipt.receipt_digest.clone(),
            contribution_id: assignment.contribution_id.clone(),
            expected_upload_reference: assignment.expected_upload_reference.clone(),
            expected_upload_manifest_digest: assignment.expected_upload_manifest_digest.clone(),
            upload_posture: FirstSwarmTrustedLanUploadPosture::PlannedOnly,
            validator_posture: FirstSwarmTrustedLanValidatorPosture::NotExecuted,
            aggregation_posture: FirstSwarmTrustedLanAggregationPosture::NotExecuted,
            replay_posture: FirstSwarmTrustedLanReplayPosture::NotExecuted,
            detail: String::from(
                "The exact contributor assignment is frozen from the live workflow plan, but the current evidence bundle records a refused attempt before remote execution began.",
            ),
        })
        .collect::<Vec<_>>();

    let stages = vec![
        stage_summary(
            "operator_bundle_materialization",
            FirstSwarmTrustedLanStageDisposition::Completed,
            bundle_materialization_ms,
            String::from(
                "The exact trusted-LAN launcher materialized the live-attempt bundle, including topology, failure-drill, and workflow-plan artifacts.",
            ),
        ),
        stage_summary(
            "live_attempt_gate",
            FirstSwarmTrustedLanStageDisposition::Refused,
            gate_wallclock_ms,
            String::from(
                "The live attempt was explicitly refused by the rehearsal no-go gate before contributor execution could begin.",
            ),
        ),
        stage_summary(
            "contributor_execution",
            FirstSwarmTrustedLanStageDisposition::Skipped,
            0,
            String::from(
                "Contributor execution never started because the live-attempt gate refused the run before the exact two-node lane could launch remote work.",
            ),
        ),
        stage_summary(
            "upload_validation_aggregation",
            FirstSwarmTrustedLanStageDisposition::Skipped,
            0,
            String::from(
                "Upload, validator, and aggregation work were skipped because no contributor execution receipts existed to process.",
            ),
        ),
        stage_summary(
            "promotion_closeout",
            FirstSwarmTrustedLanStageDisposition::Completed,
            0,
            String::from(
                "The bundle records explicit no-promotion truth instead of leaving publication posture implicit.",
            ),
        ),
    ];

    let mut bundle = FirstSwarmTrustedLanEvidenceBundle {
        schema_version: String::from(FIRST_SWARM_TRUSTED_LAN_EVIDENCE_BUNDLE_SCHEMA_VERSION),
        bundle_id: String::from("swarm.first_trusted_lan_evidence_bundle.v1"),
        run_family_id: topology.run_family_id.clone(),
        swarm_contract_digest: topology.swarm_contract_digest.clone(),
        topology_contract_digest: topology.contract_digest.clone(),
        rehearsal_report_digest: rehearsal_report.report_digest.clone(),
        failure_drills_digest: rehearsal_report.failure_drills_digest.clone(),
        workflow_plan_digest: workflow_plan.plan_digest.clone(),
        membership_receipt_digest: workflow_plan.membership_receipt.receipt_digest.clone(),
        launch_manifest_digest: if launch_receipt.manifest_digest == launch_manifest_digest {
            launch_receipt.manifest_digest.clone()
        } else {
            launch_manifest_digest
        },
        launch_receipt_digest,
        launch_status: launch_receipt.launch_status.clone(),
        live_attempt_disposition: FirstSwarmTrustedLanLiveAttemptDisposition::Refused,
        live_attempt_reason,
        contributors,
        stages,
        promotion_outcome: FirstSwarmTrustedLanPromotionOutcome {
            disposition: FirstSwarmTrustedLanPromotionDisposition::NoPromotion,
            reason: String::from(
                "The live attempt never reached contributor execution, validator disposition, or aggregation, so the lane retains explicit no-promotion truth.",
            ),
            local_snapshot_path: None,
        },
        drift_notes: vec![
            String::from(
                "This bundle is an explicit refused live-attempt bundle, not a success bundle. It preserves the exact topology, workflow plan, and expected contributor artifacts while keeping execution absent rather than faked.",
            ),
            String::from(
                "The contributor rows keep the planned upload references and manifest digests so the after-action audit can name exactly what would have been executed once the live gate is lifted.",
            ),
        ],
        claim_boundary: String::from(
            "This evidence bundle records one refused first live attempt for the exact first swarm trusted-LAN lane. It proves bundle materialization, contributor planning truth, refusal gating, replay posture, and explicit no-promotion outcome. It does not claim live two-node contributor execution, live upload/validator/aggregation receipts, or a promoted local snapshot.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = bundle.stable_digest();

    let _ = fs::remove_dir_all(&temp_bundle_dir);
    Ok(bundle)
}

/// Writes the first swarm trusted-LAN evidence bundle to one JSON path.
pub fn write_first_swarm_trusted_lan_evidence_bundle(
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmTrustedLanEvidenceBundle, FirstSwarmTrustedLanEvidenceBundleError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FirstSwarmTrustedLanEvidenceBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_first_swarm_trusted_lan_evidence_bundle()?;
    let encoded = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmTrustedLanEvidenceBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn stage_summary(
    stage_id: &str,
    disposition: FirstSwarmTrustedLanStageDisposition,
    wallclock_ms: u64,
    detail: String,
) -> FirstSwarmTrustedLanStageSummary {
    let mut summary = FirstSwarmTrustedLanStageSummary {
        stage_id: String::from(stage_id),
        disposition,
        wallclock_ms,
        detail,
        stage_digest: String::new(),
    };
    summary.stage_digest = summary.stable_digest();
    summary
}

fn temp_bundle_dir() -> PathBuf {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    std::env::temp_dir().join(format!(
        "first_swarm_trusted_lan_live_attempt_{}_{}",
        std::process::id(),
        now_ms
    ))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_repo_fixture<T>(relative_path: &str) -> Result<T, FirstSwarmTrustedLanEvidenceBundleError>
where
    T: for<'de> Deserialize<'de>,
{
    let path = repo_root().join(relative_path);
    load_json(path)
}

fn load_json<T>(path: impl AsRef<Path>) -> Result<T, FirstSwarmTrustedLanEvidenceBundleError>
where
    T: for<'de> Deserialize<'de>,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| FirstSwarmTrustedLanEvidenceBundleError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        FirstSwarmTrustedLanEvidenceBundleError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn file_sha256(path: impl AsRef<Path>) -> Result<String, FirstSwarmTrustedLanEvidenceBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| FirstSwarmTrustedLanEvidenceBundleError::Read {
        path: path.display().to_string(),
        error,
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable digest serialization"));
    hex::encode(hasher.finalize())
}
