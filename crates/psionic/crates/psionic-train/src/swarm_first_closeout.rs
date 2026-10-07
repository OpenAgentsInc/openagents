use std::{fs, path::Path, path::PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    first_swarm_run_contract, FirstSwarmRunContract, FirstSwarmTrustedLanEvidenceBundle,
    FirstSwarmTrustedLanPromotionDisposition, FirstSwarmTrustedLanRehearsalReport,
    FirstSwarmTrustedLanTopologyContract, FIRST_SWARM_TRUSTED_LAN_EVIDENCE_BUNDLE_FIXTURE_PATH,
    FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_FIXTURE_PATH,
    FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_FIXTURE_PATH,
};

/// Stable scope window for the first swarm trusted-LAN closeout report.
pub const FIRST_SWARM_TRUSTED_LAN_CLOSEOUT_SCOPE_WINDOW: &str =
    "first_swarm_trusted_lan_closeout_v1";
/// Stable schema version for the first swarm trusted-LAN closeout report.
pub const FIRST_SWARM_TRUSTED_LAN_CLOSEOUT_REPORT_SCHEMA_VERSION: &str =
    "swarm.first_trusted_lan_closeout_report.v1";
/// Stable fixture path for the first swarm trusted-LAN closeout report.
pub const FIRST_SWARM_TRUSTED_LAN_CLOSEOUT_REPORT_FIXTURE_PATH: &str =
    "fixtures/swarm/reports/first_swarm_trusted_lan_closeout_v1.json";

/// Errors surfaced while building or writing the first swarm closeout report.
#[derive(Debug, Error)]
pub enum FirstSwarmTrustedLanCloseoutError {
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
    #[error("first swarm closeout fixture contract is invalid: {detail}")]
    FixtureDrift { detail: String },
}

/// Final merge posture for the first swarm trusted-LAN closeout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanMergeDisposition {
    /// The lane did not earn accepted mergeable outputs.
    NoMerge,
    /// The lane merged accepted contributor outputs.
    Merged,
}

/// Final publish posture for the first swarm trusted-LAN closeout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSwarmTrustedLanPublishDisposition {
    /// Publication was refused.
    Refused,
    /// One local snapshot was published.
    Published,
}

/// One explicit gate checked during closeout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanCloseoutGate {
    /// Stable gate identifier.
    pub gate_id: String,
    /// Whether the gate was satisfied.
    pub satisfied: bool,
    /// Short detail for the gate.
    pub detail: String,
}

/// Publish expectation bound into the closeout report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanCloseoutPublishExpectation {
    /// Stable publisher role.
    pub publisher_role_id: String,
    /// Stable publish identifier.
    pub publish_id: String,
    /// Publish target carried by the workflow plan.
    pub target: String,
    /// Logical repository identifier carried by the workflow plan.
    pub repo_id: String,
    /// Expected local snapshot directory for an accepted mergeable run.
    pub expected_local_snapshot_directory: String,
    /// Frozen publish posture from the run contract.
    pub publish_posture: String,
    /// Exact publish surface that would be used when publication becomes truthful.
    pub publish_surface: String,
}

/// Machine-legible closeout report for the first swarm trusted-LAN lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmTrustedLanCloseoutReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable scope window.
    pub scope_window: String,
    /// Stable run family identifier.
    pub run_family_id: String,
    /// Stable swarm contract digest.
    pub swarm_contract_digest: String,
    /// Stable topology contract digest.
    pub topology_contract_digest: String,
    /// Stable rehearsal report digest.
    pub rehearsal_report_digest: String,
    /// Stable evidence bundle digest.
    pub evidence_bundle_digest: String,
    /// Stable workflow plan digest.
    pub workflow_plan_digest: String,
    /// Stable promotion posture from the contract.
    pub promotion_posture: String,
    /// Stable validator policy identifier.
    pub validator_policy_id: String,
    /// Stable aggregation policy identifier.
    pub aggregation_policy_id: String,
    /// Stable replay policy identifier.
    pub replay_policy_id: String,
    /// Stable promotion disposition retained from the evidence bundle.
    pub promotion_disposition: FirstSwarmTrustedLanPromotionDisposition,
    /// Final merge disposition.
    pub merge_disposition: FirstSwarmTrustedLanMergeDisposition,
    /// Explicit reason for the merge outcome.
    pub merge_reason: String,
    /// Final publish disposition.
    pub publish_disposition: FirstSwarmTrustedLanPublishDisposition,
    /// Explicit reason for the publish outcome.
    pub publish_reason: String,
    /// Publish expectation from the workflow plan.
    pub publish_expectation: FirstSwarmTrustedLanCloseoutPublishExpectation,
    /// Optional published snapshot path when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_snapshot_path: Option<String>,
    /// Gates checked during closeout.
    pub closeout_gates: Vec<FirstSwarmTrustedLanCloseoutGate>,
    /// Concrete truths earned by the first swarm lane.
    pub proved: Vec<String>,
    /// Surprises or observations surfaced by closeout.
    pub surprises: Vec<String>,
    /// Remaining blockers before a second truthful live attempt.
    pub remaining_blockers: Vec<String>,
    /// Exact next steps.
    pub next_steps: Vec<String>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl FirstSwarmTrustedLanCloseoutReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_first_swarm_trusted_lan_closeout_report|", &clone)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedFirstSwarmLiveWorkflowPlan {
    run_family_id: String,
    plan_digest: String,
    publish_expectation: RetainedFirstSwarmPublishExpectation,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedFirstSwarmPublishExpectation {
    publisher_role_id: String,
    publish_id: String,
    target: String,
    repo_id: String,
    expected_local_snapshot_directory: String,
    publish_posture: String,
}

/// Builds the deterministic first swarm trusted-LAN closeout report.
pub fn build_first_swarm_trusted_lan_closeout_report(
) -> Result<FirstSwarmTrustedLanCloseoutReport, FirstSwarmTrustedLanCloseoutError> {
    let contract = first_swarm_run_contract();
    let topology: FirstSwarmTrustedLanTopologyContract =
        load_repo_fixture(FIRST_SWARM_TRUSTED_LAN_TOPOLOGY_CONTRACT_FIXTURE_PATH)?;
    let rehearsal: FirstSwarmTrustedLanRehearsalReport =
        load_repo_fixture(FIRST_SWARM_TRUSTED_LAN_REHEARSAL_REPORT_FIXTURE_PATH)?;
    let evidence: FirstSwarmTrustedLanEvidenceBundle =
        load_repo_fixture(FIRST_SWARM_TRUSTED_LAN_EVIDENCE_BUNDLE_FIXTURE_PATH)?;
    let workflow_plan: RetainedFirstSwarmLiveWorkflowPlan =
        load_repo_fixture(crate::FIRST_SWARM_LIVE_WORKFLOW_PLAN_FIXTURE_PATH)?;

    validate_alignment(&contract, &topology, &rehearsal, &evidence, &workflow_plan)?;

    let publish_expectation = FirstSwarmTrustedLanCloseoutPublishExpectation {
        publisher_role_id: workflow_plan.publish_expectation.publisher_role_id.clone(),
        publish_id: workflow_plan.publish_expectation.publish_id.clone(),
        target: workflow_plan.publish_expectation.target.clone(),
        repo_id: workflow_plan.publish_expectation.repo_id.clone(),
        expected_local_snapshot_directory: workflow_plan
            .publish_expectation
            .expected_local_snapshot_directory
            .clone(),
        publish_posture: workflow_plan.publish_expectation.publish_posture.clone(),
        publish_surface: String::from(
            "psionic-mlx-workflows::MlxWorkflowWorkspace::publish_bundle",
        ),
    };

    let closeout_gates = vec![
        FirstSwarmTrustedLanCloseoutGate {
            gate_id: String::from("all_required_contributor_roles_present"),
            satisfied: false,
            detail: String::from(
                "The evidence bundle records a refused first live attempt before remote execution began, so the exact two-node lane never produced contributor execution receipts for both required roles.",
            ),
        },
        FirstSwarmTrustedLanCloseoutGate {
            gate_id: String::from("validator_acceptance_receipts_exist"),
            satisfied: false,
            detail: String::from(
                "The contributor rows remain `validator_posture = not_executed`, so no accepted validator disposition exists for merge or publication.",
            ),
        },
        FirstSwarmTrustedLanCloseoutGate {
            gate_id: String::from("replay_receipts_exist_for_accepted_contributions"),
            satisfied: false,
            detail: String::from(
                "Replay receipts remain absent because contributor execution never started and the lane requires replay receipts per accepted contribution.",
            ),
        },
        FirstSwarmTrustedLanCloseoutGate {
            gate_id: String::from("aggregation_completed_under_policy"),
            satisfied: false,
            detail: String::from(
                "Aggregation never ran under `aggregation.open_adapter.mean_delta` because no accepted contribution set existed to aggregate.",
            ),
        },
        FirstSwarmTrustedLanCloseoutGate {
            gate_id: String::from("promotion_earned_local_snapshot"),
            satisfied: false,
            detail: String::from(
                "The retained evidence bundle ends with `promotion_outcome.disposition = no_promotion`, so no local snapshot was earned for publication.",
            ),
        },
    ];

    let merge_reason = String::from(
        "No mergeable output exists because the first live attempt was refused before contributor execution, validator acceptance, replay, and aggregation could produce one accepted merged adapter state.",
    );
    let publish_reason = format!(
        "Publication is refused because the lane did not earn a mergeable local snapshot. The retained workflow plan still points at `{}` via `{}`, but that publish path stays planned-only until accepted contributor outputs, replay receipts, aggregation, and promotion truth all exist.",
        publish_expectation.expected_local_snapshot_directory,
        publish_expectation.publish_surface
    );

    let mut report = FirstSwarmTrustedLanCloseoutReport {
        schema_version: String::from(FIRST_SWARM_TRUSTED_LAN_CLOSEOUT_REPORT_SCHEMA_VERSION),
        scope_window: String::from(FIRST_SWARM_TRUSTED_LAN_CLOSEOUT_SCOPE_WINDOW),
        run_family_id: contract.run_family_id.clone(),
        swarm_contract_digest: contract.contract_digest.clone(),
        topology_contract_digest: topology.contract_digest.clone(),
        rehearsal_report_digest: rehearsal.report_digest.clone(),
        evidence_bundle_digest: evidence.bundle_digest.clone(),
        workflow_plan_digest: workflow_plan.plan_digest.clone(),
        promotion_posture: contract.governance.promotion_posture.clone(),
        validator_policy_id: contract.governance.validator_policy_id.clone(),
        aggregation_policy_id: contract.governance.aggregation_policy_id.clone(),
        replay_policy_id: contract.governance.replay_policy_id.clone(),
        promotion_disposition: evidence.promotion_outcome.disposition,
        merge_disposition: FirstSwarmTrustedLanMergeDisposition::NoMerge,
        merge_reason,
        publish_disposition: FirstSwarmTrustedLanPublishDisposition::Refused,
        publish_reason,
        publish_expectation,
        published_snapshot_path: None,
        closeout_gates,
        proved: vec![
            String::from(
                "The first swarm lane now closes out with one deterministic machine-legible no-merge and no-publish report instead of leaving the end state implicit.",
            ),
            String::from(
                "The closeout stays bound to the exact swarm contract, trusted-LAN topology contract, rehearsal report, retained evidence bundle, and workflow publish expectation for the Mac-plus-RTX-4080 lane.",
            ),
            String::from(
                "The report names the exact local snapshot surface that would be used once the lane earns mergeable outputs: `psionic-mlx-workflows::MlxWorkflowWorkspace::publish_bundle` targeting one local Hugging Face-style snapshot directory.",
            ),
        ],
        surprises: vec![
            String::from(
                "The publish surface itself is not the blocker for the first swarm lane. The blocker is still the missing live contributor, validator, replay, and aggregation receipt set.",
            ),
            String::from(
                "The lane already has enough retained planning truth to name the exact publish directory and repo identifier without widening the claim into a fake published snapshot.",
            ),
        ],
        remaining_blockers: vec![
            String::from(
                "Earn one real two-node contributor execution receipt set for the exact trusted-LAN lane.",
            ),
            String::from(
                "Retain validator acceptance, replay, and aggregation receipts for both contributors under the frozen swarm governance contract.",
            ),
            String::from(
                "Earn `promotion_outcome = promoted` before attempting local snapshot publication.",
            ),
        ],
        next_steps: vec![
            String::from(
                "Lift the current `no_go` gate only after the exact two-node lane can emit live contributor execution receipts instead of a refused bundle.",
            ),
            String::from(
                "Wire accepted contributor outputs through the existing merge and aggregation path, then re-run closeout to determine whether a local snapshot is actually earned.",
            ),
            String::from(
                "Only after merge and promotion are truthful should the Mac publisher role call `psionic-mlx-workflows::MlxWorkflowWorkspace::publish_bundle` for `first-swarm-local-snapshot`.",
            ),
        ],
        claim_boundary: String::from(
            "This closeout report records the end state of the current first swarm trusted-LAN attempt. It proves explicit no-merge truth, explicit publish-refused truth, and the exact local publish surface that would be used after a later accepted mergeable outcome. It does not claim a successful two-node live execution, accepted contributor receipts, aggregation completion, or a published local snapshot.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    Ok(report)
}

/// Writes the first swarm trusted-LAN closeout report to one JSON path.
pub fn write_first_swarm_trusted_lan_closeout_report(
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmTrustedLanCloseoutReport, FirstSwarmTrustedLanCloseoutError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FirstSwarmTrustedLanCloseoutError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_first_swarm_trusted_lan_closeout_report()?;
    let encoded = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmTrustedLanCloseoutError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn validate_alignment(
    contract: &FirstSwarmRunContract,
    topology: &FirstSwarmTrustedLanTopologyContract,
    rehearsal: &FirstSwarmTrustedLanRehearsalReport,
    evidence: &FirstSwarmTrustedLanEvidenceBundle,
    workflow_plan: &RetainedFirstSwarmLiveWorkflowPlan,
) -> Result<(), FirstSwarmTrustedLanCloseoutError> {
    if topology.swarm_contract_digest != contract.contract_digest
        || evidence.swarm_contract_digest != contract.contract_digest
    {
        return Err(FirstSwarmTrustedLanCloseoutError::FixtureDrift {
            detail: String::from("swarm contract digest drifted across retained swarm artifacts"),
        });
    }
    if topology.run_family_id != contract.run_family_id
        || rehearsal.run_family_id != contract.run_family_id
        || evidence.run_family_id != contract.run_family_id
        || workflow_plan.run_family_id != contract.run_family_id
    {
        return Err(FirstSwarmTrustedLanCloseoutError::FixtureDrift {
            detail: String::from("run family drifted across retained swarm artifacts"),
        });
    }
    if rehearsal.topology_used.topology_contract_digest != topology.contract_digest
        || evidence.topology_contract_digest != topology.contract_digest
    {
        return Err(FirstSwarmTrustedLanCloseoutError::FixtureDrift {
            detail: String::from(
                "topology contract digest drifted across retained swarm artifacts",
            ),
        });
    }
    if rehearsal.recommendation != crate::FirstSwarmTrustedLanGoNoGoRecommendation::NoGo {
        return Err(FirstSwarmTrustedLanCloseoutError::FixtureDrift {
            detail: String::from("closeout requires the retained rehearsal report to stay `no_go`"),
        });
    }
    if evidence.promotion_outcome.disposition
        != FirstSwarmTrustedLanPromotionDisposition::NoPromotion
    {
        return Err(FirstSwarmTrustedLanCloseoutError::FixtureDrift {
            detail: String::from(
                "closeout currently requires the retained evidence bundle to stay `no_promotion`",
            ),
        });
    }
    if workflow_plan.publish_expectation.publish_posture != contract.governance.publish_posture {
        return Err(FirstSwarmTrustedLanCloseoutError::FixtureDrift {
            detail: String::from(
                "workflow publish posture drifted from the swarm governance contract",
            ),
        });
    }
    if workflow_plan
        .publish_expectation
        .expected_local_snapshot_directory
        .trim()
        .is_empty()
    {
        return Err(FirstSwarmTrustedLanCloseoutError::FixtureDrift {
            detail: String::from(
                "workflow publish expectation must retain one local snapshot directory",
            ),
        });
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_repo_fixture<T>(relative_path: &str) -> Result<T, FirstSwarmTrustedLanCloseoutError>
where
    T: for<'de> Deserialize<'de>,
{
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| FirstSwarmTrustedLanCloseoutError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| FirstSwarmTrustedLanCloseoutError::Deserialize {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable digest serialization"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_first_swarm_trusted_lan_closeout_report,
        write_first_swarm_trusted_lan_closeout_report, FirstSwarmTrustedLanMergeDisposition,
        FirstSwarmTrustedLanPublishDisposition,
        FIRST_SWARM_TRUSTED_LAN_CLOSEOUT_REPORT_FIXTURE_PATH,
    };

    #[test]
    fn first_swarm_closeout_stays_no_merge_and_no_publish() {
        let report = build_first_swarm_trusted_lan_closeout_report()
            .expect("first swarm closeout report should build");
        assert_eq!(
            report.merge_disposition,
            FirstSwarmTrustedLanMergeDisposition::NoMerge
        );
        assert_eq!(
            report.publish_disposition,
            FirstSwarmTrustedLanPublishDisposition::Refused
        );
        assert_eq!(
            report.publish_expectation.publish_id,
            "first-swarm-local-snapshot"
        );
        assert!(report.published_snapshot_path.is_none());
    }

    #[test]
    fn retained_first_swarm_closeout_fixture_matches_builder() {
        let fixture: super::FirstSwarmTrustedLanCloseoutReport =
            super::load_repo_fixture(FIRST_SWARM_TRUSTED_LAN_CLOSEOUT_REPORT_FIXTURE_PATH)
                .expect("fixture should decode");
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let rebuilt = write_first_swarm_trusted_lan_closeout_report(
            temp_dir
                .path()
                .join("first_swarm_trusted_lan_closeout_report.json"),
        )
        .expect("closeout report should write");
        assert_eq!(fixture, rebuilt);
    }
}
