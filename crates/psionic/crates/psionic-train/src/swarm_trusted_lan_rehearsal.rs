use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FirstSwarmTrustedLanFailureDrillBundle, FirstSwarmTrustedLanTopologyContract,
    SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH, SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
};

/// Stable schema version for the first swarm trusted-LAN rehearsal report.
pub const FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_SCHEMA_VERSION: &str =
    "swarm.first_trusted_lan_rehearsal_report.v1";
/// Stable fixture path for the first swarm trusted-LAN rehearsal report.
pub const FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_FIXTURE_PATH: &str =
    "fixtures/swarm/reports/first_swarm_trusted_lan_rehearsal_v1.json";

const FIRST_SWARM_SIMULATED_MAC_EXECUTION_MS: u64 = 4_300;
const FIRST_SWARM_SIMULATED_LINUX_EXECUTION_MS: u64 = 6_200;
const FIRST_SWARM_SIMULATED_MAC_UPLOAD_MS: u64 = 600;
const FIRST_SWARM_SIMULATED_LINUX_UPLOAD_MS: u64 = 1_700;
const FIRST_SWARM_SIMULATED_VALIDATOR_MS: u64 = 900;
const FIRST_SWARM_SIMULATED_AGGREGATION_MS: u64 = 400;

/// Errors surfaced while building or writing the first trusted-LAN rehearsal report.
#[derive(Debug, Error)]
pub enum FirstSwarmTrustedLanRehearsalError {
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
    #[error("failed to execute first swarm trusted-LAN launcher: {detail}")]
    LauncherFailure { detail: String },
}

/// Evidence posture for one rehearsal phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanRehearsalEvidencePosture {
    /// Phase timing came from a real local rehearsal step or retained report.
    Measured,
    /// Phase timing is a projection and not yet a live receipt.
    Simulated,
}

/// Parallelization posture for one rehearsal phase or bottleneck.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanParallelizationPosture {
    /// Phase is currently serial.
    Serial,
    /// Phase is parallelizable across contributors.
    Parallelizable,
    /// Phase has both parallel and serial components.
    Mixed,
}

/// Severity of one bottleneck in the rehearsal report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanBottleneckSeverity {
    /// Blocks or dominates the current attempt.
    High,
    /// Meaningful but not the top stop condition.
    Medium,
}

/// Final recommendation from the rehearsal report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanGoNoGoRecommendation {
    /// The exact first live attempt should not be described as ready yet.
    NoGo,
    /// The exact first live attempt is acceptable under the frozen caveats.
    Go,
}

/// One node frozen into the rehearsal report's topology summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanTopologySummary {
    /// Stable topology contract digest.
    pub topology_contract_digest: String,
    /// Stable cluster namespace.
    pub cluster_namespace: String,
    /// Stable coordinator node id.
    pub coordinator_node_id: String,
    /// Stable contributor node ids.
    pub contributor_node_ids: Vec<String>,
}

/// One backend-specific timing row for the rehearsal report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanWorkerTiming {
    /// Stable node identifier.
    pub node_id: String,
    /// Stable role identifier.
    pub role_id: String,
    /// Stable execution backend label.
    pub backend_label: String,
    /// Whether the timing is measured or simulated.
    pub evidence_posture: FirstSwarmTrustedLanRehearsalEvidencePosture,
    /// Active work time for the phase.
    pub active_ms: u64,
    /// Idle time introduced by skew when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_ms: Option<u64>,
    /// Contributor skew when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skew_ms: Option<u64>,
    /// Short detail about the timing row.
    pub detail: String,
}

/// One measured or simulated phase in the trusted-LAN rehearsal report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanRehearsalPhase {
    /// Stable phase identifier.
    pub phase_id: String,
    /// Short label for the phase.
    pub label: String,
    /// Whether the phase timing is measured or simulated.
    pub evidence_posture: FirstSwarmTrustedLanRehearsalEvidencePosture,
    /// Current serial or parallelizable posture.
    pub parallelization_posture: FirstSwarmTrustedLanParallelizationPosture,
    /// Wallclock attributed to the phase.
    pub wallclock_ms: u64,
    /// Backend-specific worker rows for the phase.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub worker_timings: Vec<FirstSwarmTrustedLanWorkerTiming>,
    /// Short detail explaining the phase.
    pub detail: String,
    /// Stable phase digest.
    pub phase_digest: String,
}

impl FirstSwarmTrustedLanRehearsalPhase {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.phase_digest.clear();
        stable_digest(b"psionic_first_swarm_trusted_lan_rehearsal_phase|", &clone)
    }
}

/// One bottleneck surfaced by the rehearsal report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanBottleneck {
    /// Stable bottleneck identifier.
    pub bottleneck_id: String,
    /// Phase the bottleneck belongs to.
    pub phase_id: String,
    /// Severity of the bottleneck.
    pub severity: FirstSwarmTrustedLanBottleneckSeverity,
    /// Serial or parallelizable posture for the bottleneck.
    pub parallelization_posture: FirstSwarmTrustedLanParallelizationPosture,
    /// Short detail explaining the bottleneck.
    pub detail: String,
    /// Explicit remediation or next step.
    pub remediation: String,
    /// Stable bottleneck digest.
    pub bottleneck_digest: String,
}

impl FirstSwarmTrustedLanBottleneck {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.bottleneck_digest.clear();
        stable_digest(b"psionic_first_swarm_trusted_lan_bottleneck|", &clone)
    }
}

/// Machine-legible rehearsal report for the first trusted-LAN swarm lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanRehearsalReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable scope window.
    pub scope_window: String,
    /// Stable run family id.
    pub run_family_id: String,
    /// Topology summary used for the rehearsal.
    pub topology_used: FirstSwarmTrustedLanTopologySummary,
    /// Stable failure-drill bundle digest used for the rehearsal.
    pub failure_drills_digest: String,
    /// Stable launch-manifest digest from the exact local bundle rehearsal.
    pub launch_manifest_digest: String,
    /// Stable launch-receipt digest from the exact local bundle rehearsal.
    pub launch_receipt_digest: String,
    /// Launch status observed from the rehearsal bundle.
    pub launch_status: String,
    /// Measured and simulated phases.
    pub phases: Vec<FirstSwarmTrustedLanRehearsalPhase>,
    /// Bottleneck map.
    pub bottlenecks: Vec<FirstSwarmTrustedLanBottleneck>,
    /// Remaining phases that are still serial.
    pub remaining_serial_phase_ids: Vec<String>,
    /// Concrete remaining blockers before a truthful live attempt.
    pub remaining_blockers: Vec<String>,
    /// Final go or no-go recommendation.
    pub recommendation: FirstSwarmTrustedLanGoNoGoRecommendation,
    /// Short reason for the recommendation.
    pub recommendation_reason: String,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl FirstSwarmTrustedLanRehearsalReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_first_swarm_trusted_lan_rehearsal_report|", &clone)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct FirstSwarmLaunchManifest {
    run_family_id: String,
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
struct FirstSwarmBringupTimingReport {
    report_digest: String,
    observed_wallclock_ms: u64,
}

/// Builds one machine-legible rehearsal report for the first trusted-LAN swarm lane.
pub fn build_first_swarm_trusted_lan_rehearsal_report(
) -> Result<FirstSwarmTrustedLanRehearsalReport, FirstSwarmTrustedLanRehearsalError> {
    let temp_bundle_dir = temp_bundle_dir();
    fs::create_dir_all(&temp_bundle_dir).map_err(|error| {
        FirstSwarmTrustedLanRehearsalError::CreateDir {
            path: temp_bundle_dir.display().to_string(),
            error,
        }
    })?;

    let launcher_output = Command::new("bash")
        .arg(repo_root().join("scripts/first-swarm-launch-trusted-lan.sh"))
        .arg("--run-id")
        .arg("first-swarm-trusted-lan-rehearsal")
        .arg("--bundle-dir")
        .arg(&temp_bundle_dir)
        .arg("--manifest-only")
        .current_dir(repo_root())
        .output()
        .map_err(
            |error| FirstSwarmTrustedLanRehearsalError::LauncherFailure {
                detail: error.to_string(),
            },
        )?;
    if !launcher_output.status.success() {
        return Err(FirstSwarmTrustedLanRehearsalError::LauncherFailure {
            detail: String::from_utf8_lossy(&launcher_output.stderr)
                .trim()
                .to_string(),
        });
    }

    let manifest_path = temp_bundle_dir.join("first_swarm_trusted_lan_launch_manifest.json");
    let receipt_path = temp_bundle_dir.join("first_swarm_trusted_lan_launch_receipt.json");
    let topology_path = temp_bundle_dir.join("first_swarm_trusted_lan_topology_contract_v1.json");
    let failure_drills_path = temp_bundle_dir
        .join("reports")
        .join("first_swarm_trusted_lan_failure_drills_v1.json");

    let topology: FirstSwarmTrustedLanTopologyContract = load_json(&topology_path)?;
    let failure_drills: FirstSwarmTrustedLanFailureDrillBundle = load_json(&failure_drills_path)?;
    let manifest: FirstSwarmLaunchManifest = load_json(&manifest_path)?;
    let receipt: FirstSwarmLaunchReceipt = load_json(&receipt_path)?;
    let mac_bringup: FirstSwarmBringupTimingReport =
        load_repo_fixture(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH)?;
    let linux_bringup: FirstSwarmBringupTimingReport =
        load_repo_fixture(SWARM_LINUX_4080_BRINGUP_FIXTURE_PATH)?;

    let computed_launch_manifest_digest = file_sha256(&manifest_path)?;
    let launch_receipt_digest = file_sha256(&receipt_path)?;
    let launch_manifest_digest = if receipt.manifest_digest == computed_launch_manifest_digest {
        receipt.manifest_digest.clone()
    } else {
        computed_launch_manifest_digest
    };
    let launch_wallclock_ms = receipt
        .phase_results
        .iter()
        .filter(|phase| phase.status == "completed")
        .map(|phase| phase.finished_at_ms.saturating_sub(phase.started_at_ms))
        .sum();

    let mac_node = topology
        .nodes
        .iter()
        .find(|node| node.node_id == topology.coordinator_node_id)
        .expect("coordinator node should exist");
    let linux_node = topology
        .nodes
        .iter()
        .find(|node| node.node_id == "swarm-linux-4080-a")
        .expect("linux contributor node should exist");

    let contributor_skew_ms =
        FIRST_SWARM_SIMULATED_LINUX_EXECUTION_MS - FIRST_SWARM_SIMULATED_MAC_EXECUTION_MS;
    let upload_skew_ms =
        FIRST_SWARM_SIMULATED_LINUX_UPLOAD_MS - FIRST_SWARM_SIMULATED_MAC_UPLOAD_MS;

    let phases = vec![
        rehearsal_phase(
            "operator_bundle_materialization",
            "operator bundle materialization",
            FirstSwarmTrustedLanRehearsalEvidencePosture::Measured,
            FirstSwarmTrustedLanParallelizationPosture::Serial,
            launch_wallclock_ms,
            Vec::new(),
            String::from(
                "This measured phase comes from the exact trusted-LAN launcher and covers topology-contract generation, failure-drill generation, workflow freeze, and bring-up fixture capture in one local operator bundle.",
            ),
        ),
        rehearsal_phase(
            "mac_bringup_validation",
            "mac coordinator bring-up validation",
            FirstSwarmTrustedLanRehearsalEvidencePosture::Measured,
            FirstSwarmTrustedLanParallelizationPosture::Serial,
            mac_bringup.observed_wallclock_ms,
            vec![worker_timing(
                mac_node,
                FirstSwarmTrustedLanRehearsalEvidencePosture::Measured,
                mac_bringup.observed_wallclock_ms,
                None,
                None,
                String::from(
                    "Measured from the retained Mac MLX bring-up report. This is a real same-node contributor/validator gate on the Mac host.",
                ),
            )],
            format!(
                "The retained Mac bring-up report digest `{}` stays bound to the exact swarm topology and provides one measured MLX Metal preflight for the rehearsal.",
                mac_bringup.report_digest
            ),
        ),
        rehearsal_phase(
            "linux_bringup_validation",
            "linux contributor bring-up validation",
            FirstSwarmTrustedLanRehearsalEvidencePosture::Measured,
            FirstSwarmTrustedLanParallelizationPosture::Serial,
            linux_bringup.observed_wallclock_ms,
            vec![worker_timing(
                linux_node,
                FirstSwarmTrustedLanRehearsalEvidencePosture::Measured,
                linux_bringup.observed_wallclock_ms,
                None,
                None,
                String::from(
                    "Measured from the retained Linux RTX 4080 swarm bring-up report. This remains a retained inventory plus same-node harness measurement, not a live remote node session.",
                ),
            )],
            format!(
                "The retained Linux bring-up report digest `{}` proves the same-node CUDA harness and explicit contract posture, but it is not yet a remote two-node measurement.",
                linux_bringup.report_digest
            ),
        ),
        rehearsal_phase(
            "contributor_execution_projection",
            "contributor execution projection",
            FirstSwarmTrustedLanRehearsalEvidencePosture::Simulated,
            FirstSwarmTrustedLanParallelizationPosture::Parallelizable,
            FIRST_SWARM_SIMULATED_LINUX_EXECUTION_MS,
            vec![
                worker_timing(
                    mac_node,
                    FirstSwarmTrustedLanRehearsalEvidencePosture::Simulated,
                    FIRST_SWARM_SIMULATED_MAC_EXECUTION_MS,
                    Some(contributor_skew_ms),
                    Some(contributor_skew_ms),
                    String::from(
                        "Projected from the current same-node MLX contributor gate. The Mac contributor finishes first and then waits on the slower Linux contributor in the current two-node plan.",
                    ),
                ),
                worker_timing(
                    linux_node,
                    FirstSwarmTrustedLanRehearsalEvidencePosture::Simulated,
                    FIRST_SWARM_SIMULATED_LINUX_EXECUTION_MS,
                    Some(0),
                    Some(contributor_skew_ms),
                    String::from(
                        "Projected from the current same-node CUDA contributor harness and the first trusted-LAN role split. This is the slower contributor in the rehearsal model.",
                    ),
                ),
            ],
            String::from(
                "This phase is simulated. The repo does not yet retain a live two-node contributor execution receipt for the exact lane, so the report keeps the backend split explicit instead of pretending the execution timing is already measured.",
            ),
        ),
        rehearsal_phase(
            "artifact_upload_projection",
            "artifact upload and staging projection",
            FirstSwarmTrustedLanRehearsalEvidencePosture::Simulated,
            FirstSwarmTrustedLanParallelizationPosture::Mixed,
            FIRST_SWARM_SIMULATED_LINUX_UPLOAD_MS,
            vec![
                worker_timing(
                    mac_node,
                    FirstSwarmTrustedLanRehearsalEvidencePosture::Simulated,
                    FIRST_SWARM_SIMULATED_MAC_UPLOAD_MS,
                    Some(upload_skew_ms),
                    Some(upload_skew_ms),
                    String::from(
                        "The Mac upload path can complete earlier, but the validator still waits for the slower Linux upload before it can score a comparable contribution set.",
                    ),
                ),
                worker_timing(
                    linux_node,
                    FirstSwarmTrustedLanRehearsalEvidencePosture::Simulated,
                    FIRST_SWARM_SIMULATED_LINUX_UPLOAD_MS,
                    Some(0),
                    Some(upload_skew_ms),
                    String::from(
                        "The Linux upload path is projected as the slower staging path in the current rehearsal model and therefore dictates validator start time.",
                    ),
                ),
            ],
            String::from(
                "This phase is simulated from the exact workflow plan, expected upload digests, and current failure drills. The lane still lacks live two-node upload receipts for the trusted-LAN run.",
            ),
        ),
        rehearsal_phase(
            "validator_aggregation_projection",
            "validator and aggregation projection",
            FirstSwarmTrustedLanRehearsalEvidencePosture::Simulated,
            FirstSwarmTrustedLanParallelizationPosture::Serial,
            FIRST_SWARM_SIMULATED_VALIDATOR_MS + FIRST_SWARM_SIMULATED_AGGREGATION_MS,
            vec![worker_timing(
                mac_node,
                FirstSwarmTrustedLanRehearsalEvidencePosture::Simulated,
                FIRST_SWARM_SIMULATED_VALIDATOR_MS + FIRST_SWARM_SIMULATED_AGGREGATION_MS,
                None,
                None,
                String::from(
                    "The first lane keeps validator and aggregation work on the Mac coordinator. That remains one serial coordinator phase in the rehearsal and one real blocker before a faster live lane exists.",
                ),
            )],
            String::from(
                "Validator and aggregation timing remain simulated because the exact lane still lacks a live two-node contribution set to score and merge.",
            ),
        ),
    ];

    let bottlenecks = vec![
        bottleneck(
            "operator_bundle_materialization",
            FirstSwarmTrustedLanBottleneckSeverity::High,
            FirstSwarmTrustedLanParallelizationPosture::Serial,
            String::from(
                "The measured local rehearsal still spends most of its wallclock in serial operator bundle materialization and cargo-driven contract generation.",
            ),
            String::from(
                "Keep this as an operator rehearsal cost only. Do not mistake it for live contributor runtime, and avoid calling the lane low-friction until the bundle path stops dominating iteration time.",
            ),
        ),
        bottleneck(
            "contributor_execution_projection",
            FirstSwarmTrustedLanBottleneckSeverity::Medium,
            FirstSwarmTrustedLanParallelizationPosture::Parallelizable,
            format!(
                "The current backend-specific execution projection leaves the Mac contributor idle for {} ms while the Linux contributor finishes.",
                contributor_skew_ms
            ),
            String::from(
                "Either reduce Linux-side work per window or accept that the first live attempt is a bottleneck-mapping exercise rather than a speedup claim.",
            ),
        ),
        bottleneck(
            "validator_aggregation_projection",
            FirstSwarmTrustedLanBottleneckSeverity::High,
            FirstSwarmTrustedLanParallelizationPosture::Serial,
            String::from(
                "Validator and aggregation work remain single-host coordinator phases on the Mac node.",
            ),
            String::from(
                "Do not claim parallel end-to-end training throughput until validator and aggregation timing are measured against a real two-node contribution set.",
            ),
        ),
    ];

    let remaining_serial_phase_ids = phases
        .iter()
        .filter(|phase| {
            phase.parallelization_posture == FirstSwarmTrustedLanParallelizationPosture::Serial
        })
        .map(|phase| phase.phase_id.clone())
        .collect::<Vec<_>>();

    let mut report = FirstSwarmTrustedLanRehearsalReport {
        schema_version: String::from(FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_SCHEMA_VERSION),
        scope_window: String::from("first_swarm_trusted_lan_rehearsal_v1"),
        run_family_id: manifest.run_family_id,
        topology_used: FirstSwarmTrustedLanTopologySummary {
            topology_contract_digest: topology.contract_digest,
            cluster_namespace: topology.cluster_namespace,
            coordinator_node_id: topology.coordinator_node_id,
            contributor_node_ids: topology.contributor_node_ids,
        },
        failure_drills_digest: failure_drills.bundle_digest,
        launch_manifest_digest,
        launch_receipt_digest,
        launch_status: receipt.launch_status,
        phases,
        bottlenecks,
        remaining_serial_phase_ids,
        remaining_blockers: vec![
            String::from(
                "The exact first swarm lane still lacks a live two-node contributor execution receipt across the trusted-LAN topology.",
            ),
            String::from(
                "Upload, validator, and aggregation timings are still simulated because no live comparable contribution set has been retained yet.",
            ),
            String::from(
                "The Linux node still depends on a retained RTX 4080 inventory report rather than a live remote machine probe owned by the trusted-LAN launcher.",
            ),
            String::from(
                "No promotion or explicit no-promotion receipt exists yet for the exact two-node lane.",
            ),
        ],
        recommendation: FirstSwarmTrustedLanGoNoGoRecommendation::NoGo,
        recommendation_reason: String::from(
            "Do not describe the first live attempt as ready yet. The exact trusted-LAN topology, launch bundle, and failure drills are now frozen, but the actual two-node contributor, upload, validator, and aggregation path is still not retained as live machine-legible evidence.",
        ),
        claim_boundary: String::from(
            "This report is a rehearsal-grade bottleneck report for the exact first swarm trusted-LAN lane. It combines measured local operator-bundle and bring-up timing with explicit simulated contributor, upload, validator, and aggregation timing. It is not a live two-node swarm evidence bundle.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();

    let _ = fs::remove_dir_all(&temp_bundle_dir);
    Ok(report)
}

/// Writes the first trusted-LAN rehearsal report to one JSON path.
pub fn write_first_swarm_trusted_lan_rehearsal_report(
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmTrustedLanRehearsalReport, FirstSwarmTrustedLanRehearsalError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FirstSwarmTrustedLanRehearsalError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_first_swarm_trusted_lan_rehearsal_report()?;
    let encoded = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmTrustedLanRehearsalError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn rehearsal_phase(
    phase_id: &str,
    label: &str,
    evidence_posture: FirstSwarmTrustedLanRehearsalEvidencePosture,
    parallelization_posture: FirstSwarmTrustedLanParallelizationPosture,
    wallclock_ms: u64,
    worker_timings: Vec<FirstSwarmTrustedLanWorkerTiming>,
    detail: String,
) -> FirstSwarmTrustedLanRehearsalPhase {
    let mut phase = FirstSwarmTrustedLanRehearsalPhase {
        phase_id: String::from(phase_id),
        label: String::from(label),
        evidence_posture,
        parallelization_posture,
        wallclock_ms,
        worker_timings,
        detail,
        phase_digest: String::new(),
    };
    phase.phase_digest = phase.stable_digest();
    phase
}

fn worker_timing(
    node: &crate::FirstSwarmTrustedLanNodeContract,
    evidence_posture: FirstSwarmTrustedLanRehearsalEvidencePosture,
    active_ms: u64,
    idle_ms: Option<u64>,
    skew_ms: Option<u64>,
    detail: String,
) -> FirstSwarmTrustedLanWorkerTiming {
    FirstSwarmTrustedLanWorkerTiming {
        node_id: node.node_id.clone(),
        role_id: node.role_id.clone(),
        backend_label: node.backend_label.clone(),
        evidence_posture,
        active_ms,
        idle_ms,
        skew_ms,
        detail,
    }
}

fn bottleneck(
    phase_id: &str,
    severity: FirstSwarmTrustedLanBottleneckSeverity,
    parallelization_posture: FirstSwarmTrustedLanParallelizationPosture,
    detail: String,
    remediation: String,
) -> FirstSwarmTrustedLanBottleneck {
    let mut bottleneck = FirstSwarmTrustedLanBottleneck {
        bottleneck_id: format!("first-swarm-trusted-lan-bottleneck:{phase_id}"),
        phase_id: String::from(phase_id),
        severity,
        parallelization_posture,
        detail,
        remediation,
        bottleneck_digest: String::new(),
    };
    bottleneck.bottleneck_digest = bottleneck.stable_digest();
    bottleneck
}

fn temp_bundle_dir() -> PathBuf {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    std::env::temp_dir().join(format!(
        "first_swarm_trusted_lan_rehearsal_{}_{}",
        std::process::id(),
        now_ms
    ))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_repo_fixture<T>(relative_path: &str) -> Result<T, FirstSwarmTrustedLanRehearsalError>
where
    T: for<'de> Deserialize<'de>,
{
    let path = repo_root().join(relative_path);
    load_json(path)
}

fn load_json<T>(path: impl AsRef<Path>) -> Result<T, FirstSwarmTrustedLanRehearsalError>
where
    T: for<'de> Deserialize<'de>,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| FirstSwarmTrustedLanRehearsalError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        FirstSwarmTrustedLanRehearsalError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn file_sha256(path: impl AsRef<Path>) -> Result<String, FirstSwarmTrustedLanRehearsalError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| FirstSwarmTrustedLanRehearsalError::Read {
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
