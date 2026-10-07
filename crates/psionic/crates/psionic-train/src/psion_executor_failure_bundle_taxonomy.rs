use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_continue_restart_policy_packet,
    builtin_executor_local_cluster_review_workflow_packet,
    builtin_executor_mandatory_live_metrics_packet, PsionExecutorContinueRestartPolicyError,
    PsionExecutorLocalClusterReviewWorkflowError, PsionExecutorMandatoryLiveMetricsError,
    PSION_EXECUTOR_CONTINUE_RESTART_POLICY_DOC_PATH,
    PSION_EXECUTOR_CONTINUE_RESTART_POLICY_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH,
    PSION_EXECUTOR_MANDATORY_LIVE_METRICS_DOC_PATH,
    PSION_EXECUTOR_MANDATORY_LIVE_METRICS_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_SCHEMA_VERSION: &str =
    "psion.executor.failure_bundle_taxonomy.v1";
pub const PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_failure_bundle_taxonomy_v1.json";
pub const PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY.md";

const TAXONOMY_ID: &str = "psion_executor_failure_bundle_taxonomy_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";

#[derive(Debug, Error)]
pub enum PsionExecutorFailureBundleTaxonomyError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to parse `{path}`: {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    LiveMetrics(#[from] PsionExecutorMandatoryLiveMetricsError),
    #[error(transparent)]
    IncidentPolicy(#[from] PsionExecutorContinueRestartPolicyError),
    #[error(transparent)]
    ReviewWorkflow(#[from] PsionExecutorLocalClusterReviewWorkflowError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorFailureBundleTaxonomyRow {
    pub bundle_type_id: String,
    pub default_owner_role: String,
    pub continue_restart_posture: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorFailureBundleEmissionRow {
    pub emission_id: String,
    pub row_id: String,
    pub active_bundle_type: String,
    pub emission_status: String,
    pub review_requirement: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorFailureBundleTaxonomyPacket {
    pub schema_version: String,
    pub taxonomy_id: String,
    pub live_metrics_ref: String,
    pub live_metrics_digest: String,
    pub incident_policy_ref: String,
    pub incident_policy_digest: String,
    pub review_workflow_ref: String,
    pub review_workflow_digest: String,
    pub taxonomy_rows: Vec<PsionExecutorFailureBundleTaxonomyRow>,
    pub emitted_rows: Vec<PsionExecutorFailureBundleEmissionRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorFailureBundleTaxonomyRow {
    fn validate(&self) -> Result<(), PsionExecutorFailureBundleTaxonomyError> {
        for (field, value) in [
            (
                "psion_executor_failure_bundle_taxonomy.taxonomy_rows[].bundle_type_id",
                self.bundle_type_id.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.taxonomy_rows[].default_owner_role",
                self.default_owner_role.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.taxonomy_rows[].continue_restart_posture",
                self.continue_restart_posture.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.taxonomy_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.taxonomy_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_taxonomy_row_digest(self) != self.row_digest {
            return Err(PsionExecutorFailureBundleTaxonomyError::DigestMismatch {
                field: String::from(
                    "psion_executor_failure_bundle_taxonomy.taxonomy_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorFailureBundleEmissionRow {
    fn validate(
        &self,
        taxonomy_types: &[String],
    ) -> Result<(), PsionExecutorFailureBundleTaxonomyError> {
        for (field, value) in [
            (
                "psion_executor_failure_bundle_taxonomy.emitted_rows[].emission_id",
                self.emission_id.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.emitted_rows[].row_id",
                self.row_id.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.emitted_rows[].active_bundle_type",
                self.active_bundle_type.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.emitted_rows[].emission_status",
                self.emission_status.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.emitted_rows[].review_requirement",
                self.review_requirement.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.emitted_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.emitted_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.active_bundle_type != "no_active_failure_bundle"
            && !taxonomy_types
                .iter()
                .any(|bundle_type| bundle_type == &self.active_bundle_type)
        {
            return Err(PsionExecutorFailureBundleTaxonomyError::InvalidValue {
                field: String::from(
                    "psion_executor_failure_bundle_taxonomy.emitted_rows[].active_bundle_type",
                ),
                detail: format!("unknown bundle type `{}`", self.active_bundle_type),
            });
        }
        if stable_emission_row_digest(self) != self.row_digest {
            return Err(PsionExecutorFailureBundleTaxonomyError::DigestMismatch {
                field: String::from(
                    "psion_executor_failure_bundle_taxonomy.emitted_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorFailureBundleTaxonomyPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorFailureBundleTaxonomyError> {
        if self.schema_version != PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_SCHEMA_VERSION {
            return Err(PsionExecutorFailureBundleTaxonomyError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_failure_bundle_taxonomy.taxonomy_id",
                self.taxonomy_id.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.live_metrics_ref",
                self.live_metrics_ref.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.live_metrics_digest",
                self.live_metrics_digest.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.incident_policy_ref",
                self.incident_policy_ref.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.incident_policy_digest",
                self.incident_policy_digest.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.review_workflow_ref",
                self.review_workflow_ref.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.review_workflow_digest",
                self.review_workflow_digest.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_failure_bundle_taxonomy.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.taxonomy_rows.len() != 7 || self.emitted_rows.len() != 2 || self.support_refs.is_empty()
        {
            return Err(PsionExecutorFailureBundleTaxonomyError::InvalidValue {
                field: String::from("psion_executor_failure_bundle_taxonomy.required_counts"),
                detail: String::from(
                    "failure bundle taxonomy must stay frozen to seven taxonomy rows and two emitted rows",
                ),
            });
        }
        let taxonomy_types = self
            .taxonomy_rows
            .iter()
            .map(|row| row.bundle_type_id.clone())
            .collect::<Vec<_>>();
        for row in &self.taxonomy_rows {
            row.validate()?;
        }
        for row in &self.emitted_rows {
            row.validate(&taxonomy_types)?;
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorFailureBundleTaxonomyError::DigestMismatch {
                field: String::from("psion_executor_failure_bundle_taxonomy.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_failure_bundle_taxonomy_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorFailureBundleTaxonomyPacket, PsionExecutorFailureBundleTaxonomyError> {
    let live_metrics = builtin_executor_mandatory_live_metrics_packet(workspace_root)?;
    let incident_policy = builtin_executor_continue_restart_policy_packet(workspace_root)?;
    let review_workflow = builtin_executor_local_cluster_review_workflow_packet(workspace_root)?;

    let taxonomy_rows = vec![
        build_taxonomy_row(
            "optimizer_failure",
            "executor_lane_operator",
            "restart_from_last_green_checkpoint",
            "Optimizer instability invalidates the live training state and should restart from the last green checkpoint.",
        ),
        build_taxonomy_row(
            "batch_failure",
            "executor_lane_operator",
            "restart_after_batch_boundary_recheck",
            "Batch construction or corruption failure requires a restart after the batch boundary is revalidated.",
        ),
        build_taxonomy_row(
            "dataloader_stall",
            "review_cadence_owner",
            "hold_until_stall_source_explained",
            "Dataloader stalls require explanation before weekly review treats the run as healthy.",
        ),
        build_taxonomy_row(
            "memory_pressure",
            "executor_lane_operator",
            "restart_under_tighter_headroom",
            "Memory pressure requires a restart with restored headroom or a tighter admitted batch posture.",
        ),
        build_taxonomy_row(
            "thermal_anomaly",
            "review_cadence_owner",
            "hold_until_thermal_posture_green",
            "Thermal anomalies block clean continuation until the device returns to a declared green posture.",
        ),
        build_taxonomy_row(
            "slow_node_behavior",
            "review_cadence_owner",
            "continue_under_review_no_promotion",
            "Slow-node behavior may continue under review, but it blocks clean promotion claims.",
        ),
        build_taxonomy_row(
            "topology_failure",
            "executor_lane_operator",
            "restart_after_topology_repair",
            "Topology failure requires repair of the admitted launch or cluster shape before restart.",
        ),
    ];

    let mlx_row = live_metrics
        .metrics_rows
        .iter()
        .find(|row| row.row_id == "psion_executor_local_cluster_ledger_row_mlx_v1")
        .ok_or_else(|| PsionExecutorFailureBundleTaxonomyError::MissingField {
            field: String::from("psion_executor_failure_bundle_taxonomy.metrics_rows.mlx"),
        })?;
    let cuda_row = live_metrics
        .metrics_rows
        .iter()
        .find(|row| row.row_id == "psion_executor_local_cluster_ledger_row_4080_v1")
        .ok_or_else(|| PsionExecutorFailureBundleTaxonomyError::MissingField {
            field: String::from("psion_executor_failure_bundle_taxonomy.metrics_rows.4080"),
        })?;

    let emitted_rows = vec![
        build_emitted_row(
            "psion_executor_failure_bundle_emission_mlx_v1",
            mlx_row.row_id.as_str(),
            "no_active_failure_bundle",
            "green_no_failure_bundle",
            "review_references_current_bundle_type",
            "The retained MLX candidate row stays green and does not currently emit a failure bundle.",
        ),
        build_emitted_row(
            "psion_executor_failure_bundle_emission_4080_v1",
            cuda_row.row_id.as_str(),
            "slow_node_behavior",
            "watch_bundle_emitted",
            "review_references_current_bundle_type",
            "The retained 4080 current-best row emits `slow_node_behavior` because its live throughput posture trails the retained MLX row and must remain explicit in review.",
        ),
    ];

    let mut packet = PsionExecutorFailureBundleTaxonomyPacket {
        schema_version: String::from(PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_SCHEMA_VERSION),
        taxonomy_id: String::from(TAXONOMY_ID),
        live_metrics_ref: String::from(PSION_EXECUTOR_MANDATORY_LIVE_METRICS_FIXTURE_PATH),
        live_metrics_digest: live_metrics.packet_digest,
        incident_policy_ref: String::from(PSION_EXECUTOR_CONTINUE_RESTART_POLICY_FIXTURE_PATH),
        incident_policy_digest: incident_policy.packet_digest,
        review_workflow_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_FIXTURE_PATH),
        review_workflow_digest: review_workflow.workflow_digest,
        taxonomy_rows,
        emitted_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_MANDATORY_LIVE_METRICS_DOC_PATH),
            String::from(PSION_EXECUTOR_CONTINUE_RESTART_POLICY_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_REVIEW_WORKFLOW_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one canonical failure-bundle taxonomy. Optimizer, batch, dataloader, memory, thermal, slow-node, and topology failures have explicit bundle types, and retained ledger rows now emit their current bundle posture into the same weekly review path.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_failure_bundle_taxonomy_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorFailureBundleTaxonomyPacket, PsionExecutorFailureBundleTaxonomyError> {
    let packet = builtin_executor_failure_bundle_taxonomy_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_taxonomy_row(
    bundle_type_id: &str,
    default_owner_role: &str,
    continue_restart_posture: &str,
    detail: &str,
) -> PsionExecutorFailureBundleTaxonomyRow {
    let mut row = PsionExecutorFailureBundleTaxonomyRow {
        bundle_type_id: String::from(bundle_type_id),
        default_owner_role: String::from(default_owner_role),
        continue_restart_posture: String::from(continue_restart_posture),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_taxonomy_row_digest(&row);
    row
}

fn build_emitted_row(
    emission_id: &str,
    row_id: &str,
    active_bundle_type: &str,
    emission_status: &str,
    review_requirement: &str,
    detail: &str,
) -> PsionExecutorFailureBundleEmissionRow {
    let mut row = PsionExecutorFailureBundleEmissionRow {
        emission_id: String::from(emission_id),
        row_id: String::from(row_id),
        active_bundle_type: String::from(active_bundle_type),
        emission_status: String::from(emission_status),
        review_requirement: String::from(review_requirement),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_emission_row_digest(&row);
    row
}

fn stable_taxonomy_row_digest(row: &PsionExecutorFailureBundleTaxonomyRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_emission_row_digest(row: &PsionExecutorFailureBundleEmissionRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_packet_digest(packet: &PsionExecutorFailureBundleTaxonomyPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    digest_json(&clone)
}

fn digest_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize digest");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorFailureBundleTaxonomyError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorFailureBundleTaxonomyError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    fixture_path: &str,
    value: &T,
) -> Result<(), PsionExecutorFailureBundleTaxonomyError> {
    let path = workspace_root.join(fixture_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorFailureBundleTaxonomyError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(&path, payload).map_err(|error| PsionExecutorFailureBundleTaxonomyError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json_fixture<T: DeserializeOwned>(
    workspace_root: &Path,
    fixture_path: &str,
) -> Result<T, PsionExecutorFailureBundleTaxonomyError> {
    let path = workspace_root.join(fixture_path);
    let payload = fs::read_to_string(&path).map_err(|error| {
        PsionExecutorFailureBundleTaxonomyError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_str(&payload).map_err(|error| PsionExecutorFailureBundleTaxonomyError::Parse {
        path: path.display().to_string(),
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
    }

    #[test]
    fn builtin_failure_bundle_taxonomy_is_valid() {
        let packet = builtin_executor_failure_bundle_taxonomy_packet(workspace_root())
            .expect("build failure bundle taxonomy");
        packet.validate().expect("packet validates");
    }

    #[test]
    fn failure_bundle_fixture_matches_committed_truth() {
        let expected = builtin_executor_failure_bundle_taxonomy_packet(workspace_root())
            .expect("build expected taxonomy");
        let fixture: PsionExecutorFailureBundleTaxonomyPacket = read_json_fixture(
            workspace_root(),
            PSION_EXECUTOR_FAILURE_BUNDLE_TAXONOMY_FIXTURE_PATH,
        )
        .expect("read committed fixture");
        assert_eq!(fixture, expected);
    }

    #[test]
    fn failure_bundle_taxonomy_keeps_seven_canonical_bundle_types() {
        let packet = builtin_executor_failure_bundle_taxonomy_packet(workspace_root())
            .expect("build failure bundle taxonomy");
        assert_eq!(packet.taxonomy_rows.len(), 7);
    }

    #[test]
    fn failure_bundle_emissions_reference_review_bundle_type() {
        let packet = builtin_executor_failure_bundle_taxonomy_packet(workspace_root())
            .expect("build failure bundle taxonomy");
        assert!(packet
            .emitted_rows
            .iter()
            .all(|row| row.review_requirement == "review_references_current_bundle_type"));
    }
}
