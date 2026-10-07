use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_local_cluster_dashboard_packet, builtin_executor_mac_export_inspection_packet,
    PsionExecutorLocalClusterDashboardError, PsionExecutorMacExportInspectionError,
    PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_SCHEMA_VERSION: &str =
    "psion.executor.unified_throughput_reporting.v1";
pub const PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_unified_throughput_reporting_v1.json";
pub const PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING.md";

const REPORT_ID: &str = "psion_executor_unified_throughput_reporting_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS.md";

#[derive(Debug, Error)]
pub enum PsionExecutorUnifiedThroughputReportingError {
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
    Dashboard(#[from] PsionExecutorLocalClusterDashboardError),
    #[error(transparent)]
    MacExport(#[from] PsionExecutorMacExportInspectionError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorUnifiedTrainingThroughputRow {
    pub panel_id: String,
    pub row_id: String,
    pub profile_id: String,
    pub candidate_status: String,
    pub observed_steps_per_second: f64,
    pub observed_samples_per_second: f64,
    pub observed_source_tokens_per_second: f64,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorUnifiedServingThroughputRow {
    pub serving_row_id: String,
    pub local_cpu_machine_class_id: String,
    pub transformer_model_id: String,
    pub reference_linear_metric_id: String,
    pub hull_cache_metric_id: String,
    pub fast_route_throughput_floor_report_digest: String,
    pub hull_cache_closure_report_digest: String,
    pub throughput_floor_green: bool,
    pub min_hull_cache_speedup_over_reference_linear: f64,
    pub max_hull_cache_remaining_gap_vs_cpu_reference: f64,
    pub replacement_publication_digest: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorUnifiedThroughputBlockRow {
    pub block_id: String,
    pub status: String,
    pub detail: String,
    pub block_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorUnifiedThroughputReportingPacket {
    pub schema_version: String,
    pub report_id: String,
    pub dashboard_ref: String,
    pub dashboard_digest: String,
    pub export_inspection_ref: String,
    pub export_inspection_digest: String,
    pub current_best_training_row: PsionExecutorUnifiedTrainingThroughputRow,
    pub candidate_training_row: PsionExecutorUnifiedTrainingThroughputRow,
    pub candidate_to_current_best_steps_ratio: f64,
    pub serving_row: PsionExecutorUnifiedServingThroughputRow,
    pub replacement_candidate_row_id: String,
    pub replacement_blocked: bool,
    pub active_replacement_block_ids: Vec<String>,
    pub block_rows: Vec<PsionExecutorUnifiedThroughputBlockRow>,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub report_digest: String,
}

impl PsionExecutorUnifiedTrainingThroughputRow {
    fn validate(&self) -> Result<(), PsionExecutorUnifiedThroughputReportingError> {
        for (field, value) in [
            (
                "psion_executor_unified_throughput_reporting.training_rows[].panel_id",
                self.panel_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.training_rows[].row_id",
                self.row_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.training_rows[].profile_id",
                self.profile_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.training_rows[].candidate_status",
                self.candidate_status.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.training_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.training_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        for (field, value) in [
            (
                "psion_executor_unified_throughput_reporting.training_rows[].observed_steps_per_second",
                self.observed_steps_per_second,
            ),
            (
                "psion_executor_unified_throughput_reporting.training_rows[].observed_samples_per_second",
                self.observed_samples_per_second,
            ),
            (
                "psion_executor_unified_throughput_reporting.training_rows[].observed_source_tokens_per_second",
                self.observed_source_tokens_per_second,
            ),
        ] {
            if value <= 0.0 {
                return Err(PsionExecutorUnifiedThroughputReportingError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("throughput values must stay positive"),
                });
            }
        }
        if stable_training_row_digest(self) != self.row_digest {
            return Err(PsionExecutorUnifiedThroughputReportingError::DigestMismatch {
                field: String::from(
                    "psion_executor_unified_throughput_reporting.training_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorUnifiedServingThroughputRow {
    fn validate(&self) -> Result<(), PsionExecutorUnifiedThroughputReportingError> {
        for (field, value) in [
            (
                "psion_executor_unified_throughput_reporting.serving_row.serving_row_id",
                self.serving_row_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.local_cpu_machine_class_id",
                self.local_cpu_machine_class_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.transformer_model_id",
                self.transformer_model_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.reference_linear_metric_id",
                self.reference_linear_metric_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.hull_cache_metric_id",
                self.hull_cache_metric_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.fast_route_throughput_floor_report_digest",
                self.fast_route_throughput_floor_report_digest.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.hull_cache_closure_report_digest",
                self.hull_cache_closure_report_digest.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.replacement_publication_digest",
                self.replacement_publication_digest.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.serving_row.row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.min_hull_cache_speedup_over_reference_linear <= 0.0
            || self.max_hull_cache_remaining_gap_vs_cpu_reference <= 0.0
        {
            return Err(PsionExecutorUnifiedThroughputReportingError::InvalidValue {
                field: String::from(
                    "psion_executor_unified_throughput_reporting.serving_row.min_hull_cache_speedup_over_reference_linear",
                ),
                detail: String::from("serving throughput metrics must stay positive"),
            });
        }
        if stable_serving_row_digest(self) != self.row_digest {
            return Err(PsionExecutorUnifiedThroughputReportingError::DigestMismatch {
                field: String::from(
                    "psion_executor_unified_throughput_reporting.serving_row.row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorUnifiedThroughputBlockRow {
    fn validate(&self) -> Result<(), PsionExecutorUnifiedThroughputReportingError> {
        for (field, value) in [
            (
                "psion_executor_unified_throughput_reporting.block_rows[].block_id",
                self.block_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.block_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.block_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.block_rows[].block_digest",
                self.block_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_block_row_digest(self) != self.block_digest {
            return Err(PsionExecutorUnifiedThroughputReportingError::DigestMismatch {
                field: String::from(
                    "psion_executor_unified_throughput_reporting.block_rows[].block_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorUnifiedThroughputReportingPacket {
    pub fn validate(&self) -> Result<(), PsionExecutorUnifiedThroughputReportingError> {
        if self.schema_version != PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_SCHEMA_VERSION {
            return Err(
                PsionExecutorUnifiedThroughputReportingError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_unified_throughput_reporting.report_id",
                self.report_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.dashboard_ref",
                self.dashboard_ref.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.dashboard_digest",
                self.dashboard_digest.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.export_inspection_ref",
                self.export_inspection_ref.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.export_inspection_digest",
                self.export_inspection_digest.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.replacement_candidate_row_id",
                self.replacement_candidate_row_id.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_unified_throughput_reporting.report_digest",
                self.report_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.candidate_to_current_best_steps_ratio <= 0.0 {
            return Err(PsionExecutorUnifiedThroughputReportingError::InvalidValue {
                field: String::from(
                    "psion_executor_unified_throughput_reporting.candidate_to_current_best_steps_ratio",
                ),
                detail: String::from("steps ratio must stay positive"),
            });
        }
        self.current_best_training_row.validate()?;
        self.candidate_training_row.validate()?;
        self.serving_row.validate()?;
        if self.block_rows.len() != 1 {
            return Err(PsionExecutorUnifiedThroughputReportingError::InvalidValue {
                field: String::from("psion_executor_unified_throughput_reporting.block_rows"),
                detail: String::from("one canonical replacement blocker row is required"),
            });
        }
        for row in &self.block_rows {
            row.validate()?;
        }
        let computed_active_block_ids = self
            .block_rows
            .iter()
            .filter(|row| row.status.starts_with("blocked_"))
            .map(|row| row.block_id.clone())
            .collect::<Vec<_>>();
        if computed_active_block_ids != self.active_replacement_block_ids {
            return Err(PsionExecutorUnifiedThroughputReportingError::InvalidValue {
                field: String::from(
                    "psion_executor_unified_throughput_reporting.active_replacement_block_ids",
                ),
                detail: String::from("active replacement block ids must mirror blocked rows"),
            });
        }
        if self.replacement_blocked != !self.active_replacement_block_ids.is_empty() {
            return Err(PsionExecutorUnifiedThroughputReportingError::InvalidValue {
                field: String::from(
                    "psion_executor_unified_throughput_reporting.replacement_blocked",
                ),
                detail: String::from("replacement_blocked must match active block ids"),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorUnifiedThroughputReportingError::MissingField {
                field: String::from("psion_executor_unified_throughput_reporting.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(
                support_ref.as_str(),
                "psion_executor_unified_throughput_reporting.support_refs[]",
            )?;
        }
        if stable_report_digest(self) != self.report_digest {
            return Err(PsionExecutorUnifiedThroughputReportingError::DigestMismatch {
                field: String::from("psion_executor_unified_throughput_reporting.report_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_unified_throughput_reporting_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorUnifiedThroughputReportingPacket, PsionExecutorUnifiedThroughputReportingError>
{
    let dashboard = builtin_executor_local_cluster_dashboard_packet(workspace_root)?;
    let export = builtin_executor_mac_export_inspection_packet(workspace_root)?;

    let current_best_training_row =
        build_training_row_from_dashboard(&dashboard.current_best_card, "current_best");
    let candidate_training_row =
        build_training_row_from_dashboard(&dashboard.candidate_card, "candidate");
    let serving_row = build_serving_row_from_export(&export);
    let replacement_block_row = build_replacement_block_row(&serving_row);

    let mut packet = PsionExecutorUnifiedThroughputReportingPacket {
        schema_version: String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_SCHEMA_VERSION),
        report_id: String::from(REPORT_ID),
        dashboard_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_FIXTURE_PATH),
        dashboard_digest: dashboard.dashboard_digest,
        export_inspection_ref: String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH),
        export_inspection_digest: export.packet_digest,
        current_best_training_row,
        candidate_training_row,
        candidate_to_current_best_steps_ratio: dashboard
            .profile_comparison
            .candidate_to_current_best_steps_ratio,
        serving_row,
        replacement_candidate_row_id: dashboard.candidate_card.row_id,
        replacement_blocked: replacement_block_row.status.starts_with("blocked_"),
        active_replacement_block_ids: if replacement_block_row.status.starts_with("blocked_") {
            vec![replacement_block_row.block_id.clone()]
        } else {
            Vec::new()
        },
        block_rows: vec![replacement_block_row],
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_DASHBOARD_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_AUTOBLOCKS_DOC_PATH),
        ],
        summary: String::from(
            "The admitted executor lane now has one canonical throughput surface that keeps training throughput and fast-route serving throughput in the same retained packet. The current-best 4080 row, the candidate MLX row, and the admitted `hull_cache` serving floor are now compared together, and replacement stays blocked automatically if serving throughput truth regresses.",
        ),
        report_digest: String::new(),
    };
    packet.report_digest = stable_report_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

pub fn write_builtin_executor_unified_throughput_reporting_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorUnifiedThroughputReportingPacket, PsionExecutorUnifiedThroughputReportingError>
{
    let packet = builtin_executor_unified_throughput_reporting_packet(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn build_training_row_from_dashboard(
    row: &crate::PsionExecutorLocalClusterDashboardRunCard,
    expected_status: &str,
) -> PsionExecutorUnifiedTrainingThroughputRow {
    let mut training_row = PsionExecutorUnifiedTrainingThroughputRow {
        panel_id: row.panel_id.clone(),
        row_id: row.row_id.clone(),
        profile_id: row.profile_id.clone(),
        candidate_status: row.candidate_status.clone(),
        observed_steps_per_second: row.throughput.observed_steps_per_second,
        observed_samples_per_second: row.throughput.observed_samples_per_second,
        observed_source_tokens_per_second: row.throughput.observed_source_tokens_per_second,
        detail: format!(
            "Training throughput for `{}` remains projected directly from the canonical dashboard card so throughput review does not drift away from the retained local-cluster row.",
            expected_status
        ),
        row_digest: String::new(),
    };
    training_row.row_digest = stable_training_row_digest(&training_row);
    training_row
}

fn build_serving_row_from_export(
    export: &crate::PsionExecutorMacExportInspectionPacket,
) -> PsionExecutorUnifiedServingThroughputRow {
    let mut serving_row = PsionExecutorUnifiedServingThroughputRow {
        serving_row_id: String::from("psion_executor_serving_throughput_row_mlx_v1"),
        local_cpu_machine_class_id: export.local_cpu_machine_class_id.clone(),
        transformer_model_id: export.transformer_model_id.clone(),
        reference_linear_metric_id: export.reference_linear_metric_id.clone(),
        hull_cache_metric_id: export.hull_cache_metric_id.clone(),
        fast_route_throughput_floor_report_digest: export
            .fast_route_throughput_floor_report_digest
            .clone(),
        hull_cache_closure_report_digest: export.hull_cache_closure_report_digest.clone(),
        throughput_floor_green: true,
        min_hull_cache_speedup_over_reference_linear: export
            .min_hull_cache_speedup_over_reference_linear,
        max_hull_cache_remaining_gap_vs_cpu_reference: export
            .max_hull_cache_remaining_gap_vs_cpu_reference,
        replacement_publication_digest: export.replacement_publication_digest.clone(),
        detail: format!(
            "Serving throughput stays anchored to `{}` on `{}` with retained `hull_cache` speedup `{:.12}` over `reference_linear` and explicit CPU drift-review continuity before replacement claims.",
            export.hull_cache_metric_id,
            export.local_cpu_machine_class_id,
            export.min_hull_cache_speedup_over_reference_linear
        ),
        row_digest: String::new(),
    };
    serving_row.row_digest = stable_serving_row_digest(&serving_row);
    serving_row
}

fn build_replacement_block_row(
    serving_row: &PsionExecutorUnifiedServingThroughputRow,
) -> PsionExecutorUnifiedThroughputBlockRow {
    let serving_regressed = !serving_row.throughput_floor_green
        || serving_row.min_hull_cache_speedup_over_reference_linear <= 1.0;
    let mut row = PsionExecutorUnifiedThroughputBlockRow {
        block_id: String::from("serving_throughput_regression_candidate"),
        status: if serving_regressed {
            String::from("blocked_serving_throughput_regression")
        } else {
            String::from("green_serving_throughput_floor")
        },
        detail: if serving_regressed {
            String::from(
                "The replacement candidate cannot advance because the admitted serving throughput floor regressed or `hull_cache` no longer beats the `reference_linear` anchor.",
            )
        } else {
            String::from(
                "The replacement candidate keeps the admitted serving throughput floor green and still beats the `reference_linear` anchor, so serving throughput does not block replacement.",
            )
        },
        block_digest: String::new(),
    };
    row.block_digest = stable_block_row_digest(&row);
    row
}

fn stable_training_row_digest(row: &PsionExecutorUnifiedTrainingThroughputRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_serving_row_digest(row: &PsionExecutorUnifiedServingThroughputRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    digest_json(&clone)
}

fn stable_block_row_digest(row: &PsionExecutorUnifiedThroughputBlockRow) -> String {
    let mut clone = row.clone();
    clone.block_digest.clear();
    digest_json(&clone)
}

fn stable_report_digest(packet: &PsionExecutorUnifiedThroughputReportingPacket) -> String {
    let mut clone = packet.clone();
    clone.report_digest.clear();
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
) -> Result<(), PsionExecutorUnifiedThroughputReportingError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorUnifiedThroughputReportingError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    fixture_path: &str,
    value: &T,
) -> Result<(), PsionExecutorUnifiedThroughputReportingError> {
    let path = workspace_root.join(fixture_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorUnifiedThroughputReportingError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let payload = serde_json::to_string_pretty(value)?;
    fs::write(&path, payload).map_err(|error| PsionExecutorUnifiedThroughputReportingError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn read_json_fixture<T: DeserializeOwned>(
    workspace_root: &Path,
    fixture_path: &str,
) -> Result<T, PsionExecutorUnifiedThroughputReportingError> {
    let path = workspace_root.join(fixture_path);
    let payload = fs::read_to_string(&path).map_err(|error| {
        PsionExecutorUnifiedThroughputReportingError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_str(&payload).map_err(|error| {
        PsionExecutorUnifiedThroughputReportingError::Parse {
            path: path.display().to_string(),
            error,
        }
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
    fn builtin_unified_throughput_reporting_packet_is_valid() {
        let packet = builtin_executor_unified_throughput_reporting_packet(workspace_root())
            .expect("build unified throughput packet");
        packet.validate().expect("packet validates");
    }

    #[test]
    fn unified_throughput_reporting_fixture_matches_committed_truth() {
        let expected = builtin_executor_unified_throughput_reporting_packet(workspace_root())
            .expect("build expected packet");
        let fixture: PsionExecutorUnifiedThroughputReportingPacket = read_json_fixture(
            workspace_root(),
            PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH,
        )
        .expect("read committed fixture");
        assert_eq!(fixture, expected);
    }

    #[test]
    fn replacement_block_row_turns_red_when_serving_floor_regresses() {
        let packet = builtin_executor_unified_throughput_reporting_packet(workspace_root())
            .expect("build packet");
        let mut serving_row = packet.serving_row.clone();
        serving_row.throughput_floor_green = false;
        serving_row.row_digest = stable_serving_row_digest(&serving_row);
        let block_row = build_replacement_block_row(&serving_row);
        assert_eq!(
            block_row.status,
            "blocked_serving_throughput_regression"
        );
    }

    #[test]
    fn candidate_steps_ratio_stays_above_one_in_committed_packet() {
        let packet = builtin_executor_unified_throughput_reporting_packet(workspace_root())
            .expect("build packet");
        assert!(packet.candidate_to_current_best_steps_ratio > 1.0);
        assert!(!packet.replacement_blocked);
    }
}
