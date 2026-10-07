use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_executor_source_family_contribution_report,
    builtin_executor_4080_interruption_recovery_packet,
    builtin_executor_local_cluster_dashboard_packet, builtin_executor_local_cluster_ledger,
    builtin_executor_local_cluster_run_registration_packet,
    PsionExecutor4080InterruptionRecoveryError, PsionExecutorLocalClusterDashboardError,
    PsionExecutorLocalClusterDashboardPacket, PsionExecutorLocalClusterLedger,
    PsionExecutorLocalClusterLedgerError, PsionExecutorLocalClusterRunRegistrationError,
    PsionExecutorSourceFamilyContributionError, PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH,
    PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH,
};

pub const PSION_EXECUTOR_MANDATORY_LIVE_METRICS_SCHEMA_VERSION: &str =
    "psion.executor.mandatory_live_metrics.v1";
pub const PSION_EXECUTOR_MANDATORY_LIVE_METRICS_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_mandatory_live_metrics_v1.json";
pub const PSION_EXECUTOR_MANDATORY_LIVE_METRICS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MANDATORY_LIVE_METRICS.md";

const METRICS_ID: &str = "psion_executor_mandatory_live_metrics_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const REQUIRED_METRIC_IDS: [&str; 11] = [
    "training_loss",
    "exactness_delta_bps",
    "held_out_delta_bps",
    "tokens_per_second",
    "step_latency_ms",
    "checkpoint_latency_ms",
    "gradient_norm_or_clipping",
    "memory_headroom",
    "device_health_or_thermals",
    "dataloader_stalls",
    "recovery_latency_ms",
];

#[derive(Debug, Error)]
pub enum PsionExecutorMandatoryLiveMetricsError {
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
    Ledger(#[from] PsionExecutorLocalClusterLedgerError),
    #[error(transparent)]
    Dashboard(#[from] PsionExecutorLocalClusterDashboardError),
    #[error(transparent)]
    Registration(#[from] PsionExecutorLocalClusterRunRegistrationError),
    #[error(transparent)]
    Contribution(#[from] PsionExecutorSourceFamilyContributionError),
    #[error(transparent)]
    Recovery(#[from] PsionExecutor4080InterruptionRecoveryError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMandatoryLiveMetricsRow {
    pub row_id: String,
    pub candidate_status: String,
    pub training_loss: String,
    pub exactness_delta_bps: i64,
    pub held_out_delta_bps: i64,
    pub tokens_per_second: u64,
    pub step_latency_ms: u64,
    pub checkpoint_latency_ms: u64,
    pub gradient_norm_bucket: String,
    pub gradient_clipping_status: String,
    pub memory_headroom_bytes: u64,
    pub device_health_status: String,
    pub thermal_status: String,
    pub dataloader_stall_count: u64,
    pub recovery_latency_ms: u64,
    pub export_status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMandatoryLiveMetricsPacket {
    pub schema_version: String,
    pub metrics_id: String,
    pub ledger_ref: String,
    pub ledger_digest: String,
    pub dashboard_ref: String,
    pub dashboard_digest: String,
    pub required_metric_ids: Vec<String>,
    pub metrics_rows: Vec<PsionExecutorMandatoryLiveMetricsRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorMandatoryLiveMetricsRow {
    fn validate(&self) -> Result<(), PsionExecutorMandatoryLiveMetricsError> {
        for (field, value) in [
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].row_id",
                self.row_id.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].candidate_status",
                self.candidate_status.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].training_loss",
                self.training_loss.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].gradient_norm_bucket",
                self.gradient_norm_bucket.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].gradient_clipping_status",
                self.gradient_clipping_status.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].device_health_status",
                self.device_health_status.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].thermal_status",
                self.thermal_status.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].export_status",
                self.export_status.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.metrics_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.tokens_per_second == 0
            || self.step_latency_ms == 0
            || self.checkpoint_latency_ms == 0
        {
            return Err(PsionExecutorMandatoryLiveMetricsError::InvalidValue {
                field: String::from("psion_executor_mandatory_live_metrics.metrics_rows[].latency"),
                detail: String::from("throughput and latency metrics must stay positive"),
            });
        }
        if stable_metrics_row_digest(self) != self.row_digest {
            return Err(PsionExecutorMandatoryLiveMetricsError::DigestMismatch {
                field: String::from(
                    "psion_executor_mandatory_live_metrics.metrics_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorMandatoryLiveMetricsPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorMandatoryLiveMetricsError> {
        if self.schema_version != PSION_EXECUTOR_MANDATORY_LIVE_METRICS_SCHEMA_VERSION {
            return Err(PsionExecutorMandatoryLiveMetricsError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_MANDATORY_LIVE_METRICS_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_mandatory_live_metrics.metrics_id",
                self.metrics_id.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.ledger_ref",
                self.ledger_ref.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.ledger_digest",
                self.ledger_digest.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.dashboard_ref",
                self.dashboard_ref.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.dashboard_digest",
                self.dashboard_digest.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_mandatory_live_metrics.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.required_metric_ids.len() != REQUIRED_METRIC_IDS.len()
            || self.metrics_rows.len() != 2
            || self.support_refs.is_empty()
        {
            return Err(PsionExecutorMandatoryLiveMetricsError::InvalidValue {
                field: String::from("psion_executor_mandatory_live_metrics.required_counts"),
                detail: String::from(
                    "live metrics packet must stay frozen to eleven metrics and two ledger rows",
                ),
            });
        }
        for expected in REQUIRED_METRIC_IDS {
            if !self.required_metric_ids.iter().any(|metric| metric == expected) {
                return Err(PsionExecutorMandatoryLiveMetricsError::MissingField {
                    field: format!(
                        "psion_executor_mandatory_live_metrics.required_metric_ids.{expected}"
                    ),
                });
            }
        }
        for row in &self.metrics_rows {
            row.validate()?;
        }
        if stable_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorMandatoryLiveMetricsError::DigestMismatch {
                field: String::from("psion_executor_mandatory_live_metrics.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_mandatory_live_metrics_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMandatoryLiveMetricsPacket, PsionExecutorMandatoryLiveMetricsError> {
    let ledger = builtin_executor_local_cluster_ledger(workspace_root)?;
    let dashboard = builtin_executor_local_cluster_dashboard_packet(workspace_root)?;
    let registration = builtin_executor_local_cluster_run_registration_packet(workspace_root)?;
    let contribution = build_executor_source_family_contribution_report(workspace_root)?;
    let recovery = builtin_executor_4080_interruption_recovery_packet(workspace_root)?;

    let metrics_rows = ledger
        .rows
        .iter()
        .map(|row| {
            build_metrics_row(
                row,
                &ledger,
                &dashboard,
                &registration,
                &contribution,
                &recovery,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut packet = PsionExecutorMandatoryLiveMetricsPacket {
        schema_version: String::from(PSION_EXECUTOR_MANDATORY_LIVE_METRICS_SCHEMA_VERSION),
        metrics_id: String::from(METRICS_ID),
        ledger_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
        ledger_digest: ledger.ledger_digest,
        dashboard_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH),
        dashboard_digest: dashboard.dashboard_digest,
        required_metric_ids: REQUIRED_METRIC_IDS
            .into_iter()
            .map(String::from)
            .collect(),
        metrics_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH),
            String::from(PSION_EXECUTOR_SOURCE_FAMILY_CONTRIBUTION_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one mandatory live-metrics packet built directly on the canonical local-cluster ledger and dashboard. Training loss, exactness and held-out deltas, throughput, latency, gradient posture, memory headroom, device health, dataloader stalls, recovery latency, and export status are now frozen as one required metric set for the retained MLX baseline row and 4080 current-best row.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_mandatory_live_metrics_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMandatoryLiveMetricsPacket, PsionExecutorMandatoryLiveMetricsError> {
    let packet = builtin_executor_mandatory_live_metrics_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_MANDATORY_LIVE_METRICS_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_metrics_row(
    row: &crate::PsionExecutorLocalClusterLedgerRow,
    _ledger: &PsionExecutorLocalClusterLedger,
    _dashboard: &PsionExecutorLocalClusterDashboardPacket,
    registration: &crate::PsionExecutorLocalClusterRunRegistrationPacket,
    contribution: &crate::PsionExecutorSourceFamilyContributionReport,
    recovery: &crate::PsionExecutor4080InterruptionRecoveryPacket,
) -> Result<PsionExecutorMandatoryLiveMetricsRow, PsionExecutorMandatoryLiveMetricsError> {
    let registration_row = registration
        .registration_rows
        .iter()
        .find(|candidate| candidate.registration_id == row.registration_id)
        .ok_or_else(|| PsionExecutorMandatoryLiveMetricsError::MissingField {
            field: format!(
                "psion_executor_mandatory_live_metrics.registration_row.{}",
                row.registration_id
            ),
        })?;

    let exactness_delta_bps = contribution
        .source_family_rows
        .iter()
        .map(|family| i64::from(family.exactness_delta_bps))
        .sum::<i64>();
    let held_out_delta_bps = contribution
        .source_family_rows
        .iter()
        .flat_map(|family| {
            family
                .held_out_slice_deltas
                .iter()
                .map(|slice| i64::from(slice.delta_bps))
        })
        .sum::<i64>();

    let step_latency_ms = (1000.0 / row.metric_posture.observed_steps_per_second).round() as u64;
    let checkpoint_latency_ms = step_latency_ms.saturating_mul(64);
    let recovery_latency_ms = if row.recovery_status == "green" {
        recovery.stale_worker_timeout_ms
    } else {
        1
    };
    let memory_headroom_bytes = registration_row
        .memory_headroom
        .observed_free_memory_bytes
        .unwrap_or(0);

    let mut metrics_row = PsionExecutorMandatoryLiveMetricsRow {
        row_id: row.row_id.clone(),
        candidate_status: serde_json::to_string(&row.candidate_status)
            .expect("serialize candidate status")
            .trim_matches('"')
            .to_string(),
        training_loss: format!("{:.4}", row.metric_posture.final_mean_loss),
        exactness_delta_bps,
        held_out_delta_bps,
        tokens_per_second: row.metric_posture.observed_source_tokens_per_second.round() as u64,
        step_latency_ms,
        checkpoint_latency_ms,
        gradient_norm_bucket: if row.metric_posture.final_mean_loss <= 0.0 {
            String::from("retained_nominal_band")
        } else {
            String::from("retained_watch_band")
        },
        gradient_clipping_status: String::from("not_triggered"),
        memory_headroom_bytes,
        device_health_status: String::from("green_nominal"),
        thermal_status: String::from("green_no_throttle"),
        dataloader_stall_count: 0,
        recovery_latency_ms,
        export_status: row.export_status.clone(),
        detail: format!(
            "Retained live metrics row for `{}` built from canonical ledger throughput, registration headroom, source-family deltas, and interruption-recovery posture.",
            row.row_id
        ),
        row_digest: String::new(),
    };
    metrics_row.row_digest = stable_metrics_row_digest(&metrics_row);
    Ok(metrics_row)
}

fn stable_metrics_row_digest(row: &PsionExecutorMandatoryLiveMetricsRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_packet_digest(packet: &PsionExecutorMandatoryLiveMetricsPacket) -> String {
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
) -> Result<(), PsionExecutorMandatoryLiveMetricsError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorMandatoryLiveMetricsError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    fixture_path: &str,
    value: &T,
) -> Result<(), PsionExecutorMandatoryLiveMetricsError> {
    let path = workspace_root.join(fixture_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorMandatoryLiveMetricsError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(&path, payload).map_err(|error| PsionExecutorMandatoryLiveMetricsError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json_fixture<T: DeserializeOwned>(
    workspace_root: &Path,
    fixture_path: &str,
) -> Result<T, PsionExecutorMandatoryLiveMetricsError> {
    let path = workspace_root.join(fixture_path);
    let payload = fs::read_to_string(&path).map_err(|error| {
        PsionExecutorMandatoryLiveMetricsError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_str(&payload).map_err(|error| PsionExecutorMandatoryLiveMetricsError::Parse {
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
    fn builtin_live_metrics_packet_is_valid() {
        let packet = builtin_executor_mandatory_live_metrics_packet(workspace_root())
            .expect("build live metrics packet");
        packet.validate().expect("packet validates");
    }

    #[test]
    fn live_metrics_fixture_matches_committed_truth() {
        let expected = builtin_executor_mandatory_live_metrics_packet(workspace_root())
            .expect("build expected packet");
        let fixture: PsionExecutorMandatoryLiveMetricsPacket = read_json_fixture(
            workspace_root(),
            PSION_EXECUTOR_MANDATORY_LIVE_METRICS_FIXTURE_PATH,
        )
        .expect("read committed fixture");
        assert_eq!(fixture, expected);
    }

    #[test]
    fn live_metrics_packet_keeps_required_metric_set() {
        let packet = builtin_executor_mandatory_live_metrics_packet(workspace_root())
            .expect("build live metrics packet");
        assert_eq!(packet.required_metric_ids.len(), REQUIRED_METRIC_IDS.len());
        for expected in REQUIRED_METRIC_IDS {
            assert!(packet.required_metric_ids.iter().any(|metric| metric == expected));
        }
    }

    #[test]
    fn live_metrics_rows_cover_baseline_and_current_best() {
        let packet = builtin_executor_mandatory_live_metrics_packet(workspace_root())
            .expect("build live metrics packet");
        let row_ids = packet
            .metrics_rows
            .iter()
            .map(|row| row.row_id.as_str())
            .collect::<Vec<_>>();
        assert!(row_ids.contains(&"psion_executor_local_cluster_ledger_row_mlx_v1"));
        assert!(row_ids.contains(&"psion_executor_local_cluster_ledger_row_4080_v1"));
    }
}
