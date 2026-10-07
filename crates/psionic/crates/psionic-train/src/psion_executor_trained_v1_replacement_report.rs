use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_long_run_rehearsal_packet, builtin_executor_mac_export_inspection_packet,
    builtin_executor_percepta_closeout_status_packet, builtin_executor_trained_v1_promotion_packet,
    builtin_executor_unified_throughput_reporting_packet, PsionExecutorLongRunRehearsalError,
    PsionExecutorMacExportInspectionError, PsionExecutorPerceptaCloseoutStatusError,
    PsionExecutorTrainedV1PromotionError, PsionExecutorUnifiedThroughputReportingError,
    PSION_EXECUTOR_LONG_RUN_REHEARSAL_DOC_PATH, PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH, PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
    PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_DOC_PATH,
    PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH,
    PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH, PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH,
    PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH, PSION_EXECUTOR_TRAINED_V1_PROMOTION_DOC_PATH,
    PSION_EXECUTOR_TRAINED_V1_PROMOTION_FIXTURE_PATH,
    PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH,
    PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_SCHEMA_VERSION: &str =
    "psion.executor.trained_v1_replacement_report.v1";
pub const PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_trained_v1_replacement_report_v1.json";
pub const PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT.md";

const REPORT_ID: &str = "psion_executor_trained_v1_replacement_report_v1";
const ARTIFACT_NAMING_POLICY_REF: &str = "docs/PSION_EXECUTOR_ARTIFACT_NAMING.md";
const EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";

#[derive(Debug, Error)]
pub enum PsionExecutorTrainedV1ReplacementReportError {
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
    #[error(
        "schema version mismatch for `{field}`: expected `{expected}` but found `{actual}`"
    )]
    SchemaVersionMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Promotion(#[from] PsionExecutorTrainedV1PromotionError),
    #[error(transparent)]
    LongRun(#[from] PsionExecutorLongRunRehearsalError),
    #[error(transparent)]
    Throughput(#[from] PsionExecutorUnifiedThroughputReportingError),
    #[error(transparent)]
    Closeout(#[from] PsionExecutorPerceptaCloseoutStatusError),
    #[error(transparent)]
    Export(#[from] PsionExecutorMacExportInspectionError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1ReplacementGateRow {
    pub gate_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1ReplacementMetricRow {
    pub metric_id: String,
    pub delta_value: f64,
    pub threshold_value: f64,
    pub status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1ReplacementOutcomeRow {
    pub outcome_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1ReplacementReport {
    pub schema_version: String,
    pub report_id: String,
    pub base_model_id: String,
    pub candidate_model_id: String,
    pub candidate_route_id: String,
    pub promotion_packet_ref: String,
    pub promotion_packet_digest: String,
    pub descriptor_ref: String,
    pub descriptor_digest: String,
    pub artifact_manifest_ref: String,
    pub artifact_manifest_digest: String,
    pub lineage_contract_ref: String,
    pub lineage_contract_digest: String,
    pub artifact_naming_policy_ref: String,
    pub preserved_gate_rows: Vec<PsionExecutorTrainedV1ReplacementGateRow>,
    pub improved_metric_rows: Vec<PsionExecutorTrainedV1ReplacementMetricRow>,
    pub outcome_rows: Vec<PsionExecutorTrainedV1ReplacementOutcomeRow>,
    pub bounded_claim_status: String,
    pub replacement_decision: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub report_digest: String,
}

impl PsionExecutorTrainedV1ReplacementGateRow {
    fn validate(&self) -> Result<(), PsionExecutorTrainedV1ReplacementReportError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].gate_id",
                self.gate_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].source_ref",
                self.source_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].source_digest",
                self.source_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.status != "green" {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].status",
                ),
                detail: String::from("all preserved gates must stay green"),
            });
        }
        if stable_gate_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTrainedV1ReplacementReportError::DigestMismatch {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1ReplacementMetricRow {
    fn validate(&self) -> Result<(), PsionExecutorTrainedV1ReplacementReportError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_replacement_report.improved_metric_rows[].metric_id",
                self.metric_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.improved_metric_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.improved_metric_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.improved_metric_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.status != "green" {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.improved_metric_rows[].status",
                ),
                detail: String::from("all improved metrics must stay green"),
            });
        }
        if self.delta_value < self.threshold_value {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: format!(
                    "psion_executor_trained_v1_replacement_report.improved_metric_rows[{}].delta_value",
                    self.metric_id
                ),
                detail: String::from("delta must clear the retained threshold"),
            });
        }
        if stable_metric_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTrainedV1ReplacementReportError::DigestMismatch {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.improved_metric_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1ReplacementOutcomeRow {
    fn validate(&self) -> Result<(), PsionExecutorTrainedV1ReplacementReportError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_replacement_report.outcome_rows[].outcome_id",
                self.outcome_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.outcome_rows[].source_ref",
                self.source_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.outcome_rows[].source_digest",
                self.source_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.outcome_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.outcome_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.outcome_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.status != "green" {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.outcome_rows[].status",
                ),
                detail: String::from("all replacement outcomes must stay green"),
            });
        }
        if stable_outcome_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTrainedV1ReplacementReportError::DigestMismatch {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.outcome_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1ReplacementReport {
    pub fn validate(
        &self,
        promotion: &crate::PsionExecutorTrainedV1PromotionPacket,
        long_run: &crate::PsionExecutorLongRunRehearsalPacket,
        throughput: &crate::PsionExecutorUnifiedThroughputReportingPacket,
        closeout: &crate::PsionExecutorPerceptaCloseoutStatusPacket,
        export: &crate::PsionExecutorMacExportInspectionPacket,
    ) -> Result<(), PsionExecutorTrainedV1ReplacementReportError> {
        if self.schema_version != PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_SCHEMA_VERSION {
            return Err(
                PsionExecutorTrainedV1ReplacementReportError::SchemaVersionMismatch {
                    field: String::from("psion_executor_trained_v1_replacement_report.schema_version"),
                    expected: String::from(
                        PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_trained_v1_replacement_report.report_id",
                self.report_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.base_model_id",
                self.base_model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.candidate_model_id",
                self.candidate_model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.candidate_route_id",
                self.candidate_route_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.promotion_packet_ref",
                self.promotion_packet_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.promotion_packet_digest",
                self.promotion_packet_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.descriptor_ref",
                self.descriptor_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.descriptor_digest",
                self.descriptor_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.artifact_manifest_ref",
                self.artifact_manifest_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.artifact_manifest_digest",
                self.artifact_manifest_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.lineage_contract_ref",
                self.lineage_contract_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.lineage_contract_digest",
                self.lineage_contract_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.artifact_naming_policy_ref",
                self.artifact_naming_policy_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.bounded_claim_status",
                self.bounded_claim_status.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.replacement_decision",
                self.replacement_decision.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.report_digest",
                self.report_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.report_id != REPORT_ID {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from("psion_executor_trained_v1_replacement_report.report_id"),
                detail: String::from("unexpected report id"),
            });
        }
        if self.base_model_id != promotion.base_model_id
            || self.candidate_model_id != promotion.candidate_model_id
            || self.candidate_route_id != promotion.candidate_route_id
        {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from("psion_executor_trained_v1_replacement_report.identity"),
                detail: String::from("replacement report identity must match the promotion packet"),
            });
        }
        for (field, actual, expected) in [
            (
                "psion_executor_trained_v1_replacement_report.promotion_packet_ref",
                self.promotion_packet_ref.as_str(),
                PSION_EXECUTOR_TRAINED_V1_PROMOTION_FIXTURE_PATH,
            ),
            (
                "psion_executor_trained_v1_replacement_report.descriptor_ref",
                self.descriptor_ref.as_str(),
                PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH,
            ),
            (
                "psion_executor_trained_v1_replacement_report.artifact_manifest_ref",
                self.artifact_manifest_ref.as_str(),
                PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH,
            ),
            (
                "psion_executor_trained_v1_replacement_report.lineage_contract_ref",
                self.lineage_contract_ref.as_str(),
                PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH,
            ),
            (
                "psion_executor_trained_v1_replacement_report.artifact_naming_policy_ref",
                self.artifact_naming_policy_ref.as_str(),
                ARTIFACT_NAMING_POLICY_REF,
            ),
        ] {
            if actual != expected {
                return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                    field: String::from(field),
                    detail: format!("expected `{expected}`"),
                });
            }
        }
        for (field, actual, expected) in [
            (
                "psion_executor_trained_v1_replacement_report.promotion_packet_digest",
                self.promotion_packet_digest.as_str(),
                promotion.packet_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.descriptor_digest",
                self.descriptor_digest.as_str(),
                promotion.descriptor_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.artifact_manifest_digest",
                self.artifact_manifest_digest.as_str(),
                promotion.artifact_manifest_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_replacement_report.lineage_contract_digest",
                self.lineage_contract_digest.as_str(),
                promotion.lineage_contract_digest.as_str(),
            ),
        ] {
            if actual != expected {
                return Err(PsionExecutorTrainedV1ReplacementReportError::DigestMismatch {
                    field: String::from(field),
                });
            }
        }
        if self.preserved_gate_rows.len() != promotion.gate_rows.len() {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.preserved_gate_rows",
                ),
                detail: String::from("replacement report must preserve every promotion gate"),
            });
        }
        let expected_gate_ids: BTreeSet<&str> = promotion
            .gate_rows
            .iter()
            .map(|row| row.gate_id.as_str())
            .collect();
        let actual_gate_ids: BTreeSet<&str> = self
            .preserved_gate_rows
            .iter()
            .map(|row| row.gate_id.as_str())
            .collect();
        if actual_gate_ids != expected_gate_ids {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].gate_id",
                ),
                detail: String::from("replacement gates drifted from promotion gates"),
            });
        }
        for row in &self.preserved_gate_rows {
            row.validate()?;
            let expected = promotion
                .gate_rows
                .iter()
                .find(|candidate| candidate.gate_id == row.gate_id)
                .ok_or_else(|| PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                    field: String::from(
                        "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].gate_id",
                    ),
                    detail: String::from("unknown preserved gate id"),
                })?;
            if row.source_ref != expected.evidence_ref
                || row.source_digest != expected.evidence_digest
            {
                return Err(PsionExecutorTrainedV1ReplacementReportError::DigestMismatch {
                    field: String::from(
                        "psion_executor_trained_v1_replacement_report.preserved_gate_rows[].source_digest",
                    ),
                });
            }
        }
        let expected_metric_ids: BTreeSet<&str> = [
            "reference_linear_delta",
            "hull_cache_delta",
            "hull_cache_speedup_improvement",
            "cpu_gap_reduction",
            "exactness_net_delta_bps",
            "held_out_delta_bps",
            "training_steps_per_second_delta",
        ]
        .into_iter()
        .collect();
        let actual_metric_ids: BTreeSet<&str> = self
            .improved_metric_rows
            .iter()
            .map(|row| row.metric_id.as_str())
            .collect();
        if actual_metric_ids != expected_metric_ids {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.improved_metric_rows[].metric_id",
                ),
                detail: String::from(
                    "replacement metrics drifted from the retained comparison set",
                ),
            });
        }
        for row in &self.improved_metric_rows {
            row.validate()?;
        }
        let expected_outcome_ids: BTreeSet<&str> = [
            "throughput_outcome_green",
            "stability_outcome_green",
            "recovery_outcome_green",
            "export_outcome_green",
            "closeout_outcome_green",
        ]
        .into_iter()
        .collect();
        let actual_outcome_ids: BTreeSet<&str> = self
            .outcome_rows
            .iter()
            .map(|row| row.outcome_id.as_str())
            .collect();
        if actual_outcome_ids != expected_outcome_ids {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.outcome_rows[].outcome_id",
                ),
                detail: String::from("replacement outcomes drifted from the retained closeout set"),
            });
        }
        for row in &self.outcome_rows {
            row.validate()?;
        }
        if self.bounded_claim_status != "green_bounded_replacement_ready" {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.bounded_claim_status",
                ),
                detail: String::from("bounded claim status must stay green and narrow"),
            });
        }
        if self.replacement_decision != "publish_trained_v1_replacement_report" {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.replacement_decision",
                ),
                detail: String::from("replacement decision must publish the bounded report"),
            });
        }
        if throughput.replacement_blocked || !throughput.serving_row.throughput_floor_green {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from("psion_executor_trained_v1_replacement_report.outcome_rows"),
                detail: String::from("throughput source packet is not green"),
            });
        }
        if !long_run.rehearsal_green {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from("psion_executor_trained_v1_replacement_report.outcome_rows"),
                detail: String::from("long-run rehearsal must stay green"),
            });
        }
        if closeout.percepta_closeout_status != "green_bounded"
            || closeout.route_replacement_truth_status != "green"
        {
            return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_replacement_report.bounded_claim_status",
                ),
                detail: String::from("bounded closeout status must stay green"),
            });
        }
        let export_checklist_ids: BTreeSet<&str> = export
            .checklist_rows
            .iter()
            .filter(|row| row.status == "green")
            .map(|row| row.checklist_id.as_str())
            .collect();
        for required in [
            "portable_bundle_roundtrip_green",
            "reference_linear_anchor_green",
            "hull_cache_fast_route_green",
            "replacement_publication_green",
        ] {
            if !export_checklist_ids.contains(required) {
                return Err(PsionExecutorTrainedV1ReplacementReportError::InvalidValue {
                    field: String::from(
                        "psion_executor_trained_v1_replacement_report.outcome_rows",
                    ),
                    detail: format!("export packet is missing `{required}`"),
                });
            }
        }
        if stable_report_digest(self) != self.report_digest {
            return Err(PsionExecutorTrainedV1ReplacementReportError::DigestMismatch {
                field: String::from("psion_executor_trained_v1_replacement_report.report_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_trained_v1_replacement_report(
    workspace_root: &Path,
) -> Result<PsionExecutorTrainedV1ReplacementReport, PsionExecutorTrainedV1ReplacementReportError>
{
    let promotion = builtin_executor_trained_v1_promotion_packet(workspace_root)?;
    let long_run = builtin_executor_long_run_rehearsal_packet(workspace_root)?;
    let throughput = builtin_executor_unified_throughput_reporting_packet(workspace_root)?;
    let closeout = builtin_executor_percepta_closeout_status_packet(workspace_root)?;
    let export = builtin_executor_mac_export_inspection_packet(workspace_root)?;

    let preserved_gate_rows = promotion
        .gate_rows
        .iter()
        .map(|row| PsionExecutorTrainedV1ReplacementGateRow {
            gate_id: row.gate_id.clone(),
            source_ref: row.evidence_ref.clone(),
            source_digest: row.evidence_digest.clone(),
            status: String::from("green"),
            detail: format!(
                "Retained promotion gate `{}` stays green on the first bounded `trained-v1` replacement report.",
                row.gate_id
            ),
            row_digest: String::new(),
        })
        .map(|mut row| {
            row.row_digest = stable_gate_row_digest(&row);
            row
        })
        .collect::<Vec<_>>();

    let improved_metric_rows = vec![
        (
            "reference_linear_delta",
            promotion.reference_linear_delta,
            promotion.reference_linear_minimum_meaningful_delta,
            format!(
                "`reference_linear` improvement `{:.6}` still clears the retained non-saturated threshold `{:.6}`.",
                promotion.reference_linear_delta,
                promotion.reference_linear_minimum_meaningful_delta
            ),
        ),
        (
            "hull_cache_delta",
            promotion.hull_cache_delta,
            promotion.hull_cache_minimum_meaningful_delta,
            format!(
                "Admitted `hull_cache` improvement `{:.6}` still clears the retained non-saturated threshold `{:.6}`.",
                promotion.hull_cache_delta,
                promotion.hull_cache_minimum_meaningful_delta
            ),
        ),
        (
            "hull_cache_speedup_improvement",
            promotion.hull_cache_speedup_improvement,
            promotion.hull_cache_speedup_floor,
            format!(
                "Fast-route speedup improvement `{:.12}` stays above the retained floor `{:.12}`.",
                promotion.hull_cache_speedup_improvement,
                promotion.hull_cache_speedup_floor
            ),
        ),
        (
            "cpu_gap_reduction",
            promotion.cpu_gap_reduction,
            promotion.cpu_gap_improvement_floor,
            format!(
                "CPU-gap reduction `{:.12}` stays above the retained floor `{:.12}`.",
                promotion.cpu_gap_reduction,
                promotion.cpu_gap_improvement_floor
            ),
        ),
        (
            "exactness_net_delta_bps",
            f64::from(promotion.exactness_net_delta_bps),
            0.0,
            format!(
                "Exactness net delta `{}` bps stays non-negative against `trained-v0`.",
                promotion.exactness_net_delta_bps
            ),
        ),
        (
            "held_out_delta_bps",
            f64::from(promotion.held_out_delta_bps),
            0.0,
            format!(
                "Held-out delta `{}` bps stays non-negative against `trained-v0`.",
                promotion.held_out_delta_bps
            ),
        ),
        (
            "training_steps_per_second_delta",
            promotion.training_steps_per_second_delta,
            0.0,
            format!(
                "Training steps-per-second delta `{:.12}` stays positive against the retained baseline.",
                promotion.training_steps_per_second_delta
            ),
        ),
    ]
    .into_iter()
    .map(|(metric_id, delta_value, threshold_value, detail)| {
        let mut row = PsionExecutorTrainedV1ReplacementMetricRow {
            metric_id: String::from(metric_id),
            delta_value,
            threshold_value,
            status: String::from("green"),
            detail,
            row_digest: String::new(),
        };
        row.row_digest = stable_metric_row_digest(&row);
        row
    })
    .collect::<Vec<_>>();

    let outcome_rows = vec![
        (
            "throughput_outcome_green",
            PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_FIXTURE_PATH,
            throughput.report_digest.clone(),
            format!(
                "Unified throughput remains green: candidate/current-best steps ratio `{:.12}`, current-best steps `{:.12}`, candidate steps `{:.12}`, serving floor green `{}`.",
                throughput.candidate_to_current_best_steps_ratio,
                throughput.current_best_training_row.observed_steps_per_second,
                throughput.candidate_training_row.observed_steps_per_second,
                throughput.serving_row.throughput_floor_green
            ),
        ),
        (
            "stability_outcome_green",
            PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH,
            long_run.packet_digest.clone(),
            format!(
                "Long-run rehearsal stays green with incident `{}` and review status `{}`.",
                long_run.incident_class_id,
                long_run.review_log.status
            ),
        ),
        (
            "recovery_outcome_green",
            long_run.interruption_recovery_ref.as_str(),
            long_run.interruption_recovery_digest.clone(),
            format!(
                "Recovery remains explicit through action `{}` from checkpoint step `{}`.",
                long_run.recovery_action,
                long_run.checkpoint_step
            ),
        ),
        (
            "export_outcome_green",
            PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
            export.packet_digest.clone(),
            format!(
                "Mac export inspection stays green on machine class `{}` with replacement publication digest `{}`.",
                export.local_cpu_machine_class_id,
                export.replacement_publication_digest
            ),
        ),
        (
            "closeout_outcome_green",
            PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH,
            closeout.packet_digest.clone(),
            format!(
                "Bounded closeout remains `{}` with workload truth `{}`, fast-path truth `{}`, and route-replacement truth `{}`.",
                closeout.percepta_closeout_status,
                closeout.workload_truth_status,
                closeout.fast_path_truth_status,
                closeout.route_replacement_truth_status
            ),
        ),
    ]
    .into_iter()
    .map(|(outcome_id, source_ref, source_digest, detail)| {
        let mut row = PsionExecutorTrainedV1ReplacementOutcomeRow {
            outcome_id: String::from(outcome_id),
            source_ref: String::from(source_ref),
            source_digest,
            status: String::from("green"),
            detail,
            row_digest: String::new(),
        };
        row.row_digest = stable_outcome_row_digest(&row);
        row
    })
    .collect::<Vec<_>>();

    let mut report = PsionExecutorTrainedV1ReplacementReport {
        schema_version: String::from(PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_SCHEMA_VERSION),
        report_id: String::from(REPORT_ID),
        base_model_id: promotion.base_model_id.clone(),
        candidate_model_id: promotion.candidate_model_id.clone(),
        candidate_route_id: promotion.candidate_route_id.clone(),
        promotion_packet_ref: String::from(PSION_EXECUTOR_TRAINED_V1_PROMOTION_FIXTURE_PATH),
        promotion_packet_digest: promotion.packet_digest.clone(),
        descriptor_ref: String::from(PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH),
        descriptor_digest: promotion.descriptor_digest.clone(),
        artifact_manifest_ref: String::from(PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH),
        artifact_manifest_digest: promotion.artifact_manifest_digest.clone(),
        lineage_contract_ref: String::from(PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH),
        lineage_contract_digest: promotion.lineage_contract_digest.clone(),
        artifact_naming_policy_ref: String::from(ARTIFACT_NAMING_POLICY_REF),
        preserved_gate_rows,
        improved_metric_rows,
        outcome_rows,
        bounded_claim_status: String::from("green_bounded_replacement_ready"),
        replacement_decision: String::from("publish_trained_v1_replacement_report"),
        support_refs: vec![
            String::from(PSION_EXECUTOR_TRAINED_V1_PROMOTION_DOC_PATH),
            String::from(PSION_EXECUTOR_UNIFIED_THROUGHPUT_REPORTING_DOC_PATH),
            String::from(PSION_EXECUTOR_LONG_RUN_REHEARSAL_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_DOC_PATH),
            String::from(EXECUTOR_PROGRAM_DOC_PATH),
            String::from(ARTIFACT_NAMING_POLICY_REF),
        ],
        summary: String::from(
            "The executor lane now has one final bounded `trained-v1` replacement report. \
             It projects the retained promotion packet, unified throughput report, \
             long-run rehearsal, Mac export inspection, and bounded closeout status \
             into one replacement verdict without widening the claim boundary beyond \
             the admitted executor workload family.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_report_digest(&report);
    report.validate(&promotion, &long_run, &throughput, &closeout, &export)?;
    Ok(report)
}

pub fn write_builtin_executor_trained_v1_replacement_report(
    workspace_root: &Path,
) -> Result<PsionExecutorTrainedV1ReplacementReport, PsionExecutorTrainedV1ReplacementReportError>
{
    let report = builtin_executor_trained_v1_replacement_report(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_FIXTURE_PATH,
        &report,
    )?;
    Ok(report)
}

fn stable_gate_row_digest(row: &PsionExecutorTrainedV1ReplacementGateRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_trained_v1_replacement_gate_row", &clone)
}

fn stable_metric_row_digest(row: &PsionExecutorTrainedV1ReplacementMetricRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_trained_v1_replacement_metric_row", &clone)
}

fn stable_outcome_row_digest(row: &PsionExecutorTrainedV1ReplacementOutcomeRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_trained_v1_replacement_outcome_row", &clone)
}

fn stable_report_digest(report: &PsionExecutorTrainedV1ReplacementReport) -> String {
    let mut clone = report.clone();
    clone.report_digest.clear();
    stable_json_digest("psion_executor_trained_v1_replacement_report", &clone)
}

fn stable_json_digest<T: Serialize>(label: &str, value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value).expect("stable json"));
    hex::encode(hasher.finalize())
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorTrainedV1ReplacementReportError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorTrainedV1ReplacementReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| {
        PsionExecutorTrainedV1ReplacementReportError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorTrainedV1ReplacementReportError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        PsionExecutorTrainedV1ReplacementReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionExecutorTrainedV1ReplacementReportError::Parse {
            path: path.display().to_string(),
            error,
        }
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorTrainedV1ReplacementReportError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorTrainedV1ReplacementReportError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn builtin_executor_trained_v1_replacement_report_is_valid(
    ) -> Result<(), PsionExecutorTrainedV1ReplacementReportError> {
        let root = workspace_root();
        let promotion = builtin_executor_trained_v1_promotion_packet(root.as_path())?;
        let long_run = builtin_executor_long_run_rehearsal_packet(root.as_path())?;
        let throughput = builtin_executor_unified_throughput_reporting_packet(root.as_path())?;
        let closeout = builtin_executor_percepta_closeout_status_packet(root.as_path())?;
        let export = builtin_executor_mac_export_inspection_packet(root.as_path())?;
        let report = builtin_executor_trained_v1_replacement_report(root.as_path())?;
        report.validate(&promotion, &long_run, &throughput, &closeout, &export)?;
        assert_eq!(report.bounded_claim_status, "green_bounded_replacement_ready");
        assert_eq!(
            report.replacement_decision,
            "publish_trained_v1_replacement_report"
        );
        Ok(())
    }

    #[test]
    fn trained_v1_replacement_report_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorTrainedV1ReplacementReportError> {
        let root = workspace_root();
        let expected: PsionExecutorTrainedV1ReplacementReport = read_json(
            root.as_path(),
            PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_trained_v1_replacement_report(root.as_path())?;
        if expected.report_digest != actual.report_digest {
            return Err(PsionExecutorTrainedV1ReplacementReportError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_TRAINED_V1_REPLACEMENT_REPORT_FIXTURE_PATH),
            });
        }
        Ok(())
    }
}
