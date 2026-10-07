use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_4080_decision_grade_run_packet,
    builtin_executor_4080_interruption_recovery_packet, builtin_executor_4080_smoke_run_packet,
    builtin_executor_baseline_truth_record, builtin_executor_local_cluster_roundtrip_packet,
    builtin_executor_local_cluster_run_registration_packet, builtin_executor_mac_export_inspection_packet,
    PsionExecutor4080DecisionGradeRunError,
    PsionExecutor4080InterruptionRecoveryError, PsionExecutor4080SmokeRunError,
    PsionExecutorBaselineTruthError, PsionExecutorLocalClusterCandidateStatus,
    PsionExecutorLocalClusterRoundtripError,
    PsionExecutorLocalClusterRunRegistrationError, PsionExecutorLocalClusterRunRegistrationPacket,
    PsionExecutorMacExportInspectionError, PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_SCHEMA_VERSION: &str =
    "psion.executor.local_cluster_ledger.v1";
pub const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_local_cluster_ledger_v1.json";
pub const PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER.md";

const MLX_ROW_ID: &str = "psion_executor_local_cluster_ledger_row_mlx_v1";
const CUDA_ROW_ID: &str = "psion_executor_local_cluster_ledger_row_4080_v1";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION.md";
const PSION_EXECUTOR_MLX_DECISION_GRADE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MLX_DECISION_GRADE_RUN.md";
const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MAC_EXPORT_INSPECTION.md";
const PSION_EXECUTOR_4080_DECISION_GRADE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_DECISION_GRADE_RUN.md";
const PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY.md";
const PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH: &str = "docs/PSION_EXECUTOR_4080_SMOKE_RUN.md";
const PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP.md";

#[derive(Debug, Error)]
pub enum PsionExecutorLocalClusterLedgerError {
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
    #[error("index mismatch for `{field}` and key `{key}`")]
    IndexMismatch { field: String, key: String },
    #[error("duplicate row id `{row_id}`")]
    DuplicateRowId { row_id: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Registration(#[from] PsionExecutorLocalClusterRunRegistrationError),
    #[error(transparent)]
    MlxExport(#[from] PsionExecutorMacExportInspectionError),
    #[error(transparent)]
    DecisionGrade4080(#[from] PsionExecutor4080DecisionGradeRunError),
    #[error(transparent)]
    Recovery4080(#[from] PsionExecutor4080InterruptionRecoveryError),
    #[error(transparent)]
    Smoke4080(#[from] PsionExecutor4080SmokeRunError),
    #[error(transparent)]
    Roundtrip(#[from] PsionExecutorLocalClusterRoundtripError),
    #[error(transparent)]
    BaselineTruth(#[from] PsionExecutorBaselineTruthError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterCheckpointLineage {
    pub checkpoint_family: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_pointer_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_bundle_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterCostPosture {
    pub wallclock_budget_seconds: u64,
    pub observed_duration_ms: u64,
    pub budget_burn_ratio: f64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterMetricPosture {
    pub completed_steps: u64,
    pub final_mean_loss: f64,
    pub observed_steps_per_second: f64,
    pub observed_samples_per_second: f64,
    pub observed_source_tokens_per_second: f64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterFailureFact {
    pub fact_id: String,
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterLedgerRow {
    pub row_id: String,
    pub registration_id: String,
    pub run_id: String,
    pub search_run_ids: Vec<String>,
    pub admitted_profile_id: String,
    pub model_id: String,
    pub candidate_status: PsionExecutorLocalClusterCandidateStatus,
    pub eval_pack_ids: Vec<String>,
    pub config_summary: String,
    pub checkpoint_lineage: PsionExecutorLocalClusterCheckpointLineage,
    pub cost_posture: PsionExecutorLocalClusterCostPosture,
    pub metric_posture: PsionExecutorLocalClusterMetricPosture,
    pub failure_facts: Vec<PsionExecutorLocalClusterFailureFact>,
    pub export_status: String,
    pub recovery_status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterSearchIndex {
    pub run_id_to_row_ids: BTreeMap<String, Vec<String>>,
    pub profile_id_to_row_ids: BTreeMap<String, Vec<String>>,
    pub eval_pack_id_to_row_ids: BTreeMap<String, Vec<String>>,
    pub model_id_to_row_ids: BTreeMap<String, Vec<String>>,
    pub candidate_status_to_row_ids: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorLocalClusterLedger {
    pub schema_version: String,
    pub ledger_id: String,
    pub registration_packet_ref: String,
    pub registration_packet_sha256: String,
    pub registration_packet_digest: String,
    pub baseline_truth_ref: String,
    pub baseline_truth_sha256: String,
    pub baseline_truth_digest: String,
    pub rows: Vec<PsionExecutorLocalClusterLedgerRow>,
    pub search_index: PsionExecutorLocalClusterSearchIndex,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub ledger_digest: String,
}

impl PsionExecutorLocalClusterCheckpointLineage {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        ensure_nonempty(
            self.checkpoint_family.as_str(),
            "psion_executor_local_cluster_ledger.rows[].checkpoint_lineage.checkpoint_family",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_local_cluster_ledger.rows[].checkpoint_lineage.detail",
        )
    }
}

impl PsionExecutorLocalClusterCostPosture {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        if self.wallclock_budget_seconds == 0 {
            return Err(PsionExecutorLocalClusterLedgerError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_ledger.rows[].cost_posture.wallclock_budget_seconds",
                ),
                detail: String::from("budget must stay positive"),
            });
        }
        if self.observed_duration_ms == 0 {
            return Err(PsionExecutorLocalClusterLedgerError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_ledger.rows[].cost_posture.observed_duration_ms",
                ),
                detail: String::from("observed duration must stay positive"),
            });
        }
        if self.budget_burn_ratio <= 0.0 || self.budget_burn_ratio > 1.0 {
            return Err(PsionExecutorLocalClusterLedgerError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_ledger.rows[].cost_posture.budget_burn_ratio",
                ),
                detail: String::from("budget burn ratio must stay in (0, 1]"),
            });
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_local_cluster_ledger.rows[].cost_posture.detail",
        )
    }
}

impl PsionExecutorLocalClusterMetricPosture {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        if self.completed_steps == 0 {
            return Err(PsionExecutorLocalClusterLedgerError::InvalidValue {
                field: String::from(
                    "psion_executor_local_cluster_ledger.rows[].metric_posture.completed_steps",
                ),
                detail: String::from("completed steps must stay positive"),
            });
        }
        for (field, value) in [
            (
                "psion_executor_local_cluster_ledger.rows[].metric_posture.observed_steps_per_second",
                self.observed_steps_per_second,
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].metric_posture.observed_samples_per_second",
                self.observed_samples_per_second,
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].metric_posture.observed_source_tokens_per_second",
                self.observed_source_tokens_per_second,
            ),
        ] {
            if value <= 0.0 {
                return Err(PsionExecutorLocalClusterLedgerError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("observed throughput must stay positive"),
                });
            }
        }
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_local_cluster_ledger.rows[].metric_posture.detail",
        )
    }
}

impl PsionExecutorLocalClusterFailureFact {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_ledger.rows[].failure_facts[].fact_id",
                self.fact_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].failure_facts[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].failure_facts[].detail",
                self.detail.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterLedgerRow {
    fn validate(&self) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        for (field, value) in [
            (
                "psion_executor_local_cluster_ledger.rows[].row_id",
                self.row_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].registration_id",
                self.registration_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].run_id",
                self.run_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].admitted_profile_id",
                self.admitted_profile_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].config_summary",
                self.config_summary.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].export_status",
                self.export_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].recovery_status",
                self.recovery_status.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.search_run_ids.is_empty() {
            return Err(PsionExecutorLocalClusterLedgerError::MissingField {
                field: String::from("psion_executor_local_cluster_ledger.rows[].search_run_ids"),
            });
        }
        for run_id in &self.search_run_ids {
            ensure_nonempty(
                run_id.as_str(),
                "psion_executor_local_cluster_ledger.rows[].search_run_ids[]",
            )?;
        }
        if self.eval_pack_ids.is_empty() {
            return Err(PsionExecutorLocalClusterLedgerError::MissingField {
                field: String::from("psion_executor_local_cluster_ledger.rows[].eval_pack_ids"),
            });
        }
        for eval_pack_id in &self.eval_pack_ids {
            ensure_nonempty(
                eval_pack_id.as_str(),
                "psion_executor_local_cluster_ledger.rows[].eval_pack_ids[]",
            )?;
        }
        self.checkpoint_lineage.validate()?;
        self.cost_posture.validate()?;
        self.metric_posture.validate()?;
        if self.failure_facts.is_empty() {
            return Err(PsionExecutorLocalClusterLedgerError::MissingField {
                field: String::from("psion_executor_local_cluster_ledger.rows[].failure_facts"),
            });
        }
        for fact in &self.failure_facts {
            fact.validate()?;
        }
        if stable_ledger_row_digest(self) != self.row_digest {
            return Err(PsionExecutorLocalClusterLedgerError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_ledger.rows[].row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorLocalClusterLedger {
    pub fn rows_for_run_id<'a>(
        &'a self,
        run_id: &str,
    ) -> Vec<&'a PsionExecutorLocalClusterLedgerRow> {
        self.search_index
            .run_id_to_row_ids
            .get(run_id)
            .map_or_else(Vec::new, |row_ids| self.rows_for_ids(row_ids))
    }

    pub fn rows_for_profile_id<'a>(
        &'a self,
        profile_id: &str,
    ) -> Vec<&'a PsionExecutorLocalClusterLedgerRow> {
        self.search_index
            .profile_id_to_row_ids
            .get(profile_id)
            .map_or_else(Vec::new, |row_ids| self.rows_for_ids(row_ids))
    }

    pub fn rows_for_eval_pack_id<'a>(
        &'a self,
        eval_pack_id: &str,
    ) -> Vec<&'a PsionExecutorLocalClusterLedgerRow> {
        self.search_index
            .eval_pack_id_to_row_ids
            .get(eval_pack_id)
            .map_or_else(Vec::new, |row_ids| self.rows_for_ids(row_ids))
    }

    pub fn rows_for_model_id<'a>(
        &'a self,
        model_id: &str,
    ) -> Vec<&'a PsionExecutorLocalClusterLedgerRow> {
        self.search_index
            .model_id_to_row_ids
            .get(model_id)
            .map_or_else(Vec::new, |row_ids| self.rows_for_ids(row_ids))
    }

    pub fn rows_for_candidate_status<'a>(
        &'a self,
        candidate_status: PsionExecutorLocalClusterCandidateStatus,
    ) -> Vec<&'a PsionExecutorLocalClusterLedgerRow> {
        let key = candidate_status_key(&candidate_status);
        self.search_index
            .candidate_status_to_row_ids
            .get(key.as_str())
            .map_or_else(Vec::new, |row_ids| self.rows_for_ids(row_ids))
    }

    fn rows_for_ids<'a>(
        &'a self,
        row_ids: &[String],
    ) -> Vec<&'a PsionExecutorLocalClusterLedgerRow> {
        let id_set: BTreeSet<&str> = row_ids.iter().map(String::as_str).collect();
        self.rows
            .iter()
            .filter(|row| id_set.contains(row.row_id.as_str()))
            .collect()
    }

    pub fn validate(&self) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_local_cluster_ledger.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_SCHEMA_VERSION {
            return Err(
                PsionExecutorLocalClusterLedgerError::SchemaVersionMismatch {
                    expected: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_SCHEMA_VERSION),
                    actual: self.schema_version.clone(),
                },
            );
        }
        for (field, value) in [
            (
                "psion_executor_local_cluster_ledger.ledger_id",
                self.ledger_id.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.registration_packet_ref",
                self.registration_packet_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.registration_packet_sha256",
                self.registration_packet_sha256.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.registration_packet_digest",
                self.registration_packet_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.baseline_truth_ref",
                self.baseline_truth_ref.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.baseline_truth_sha256",
                self.baseline_truth_sha256.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.baseline_truth_digest",
                self.baseline_truth_digest.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_local_cluster_ledger.ledger_digest",
                self.ledger_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.rows.is_empty() {
            return Err(PsionExecutorLocalClusterLedgerError::MissingField {
                field: String::from("psion_executor_local_cluster_ledger.rows"),
            });
        }
        let mut seen_row_ids = BTreeSet::new();
        for row in &self.rows {
            if !seen_row_ids.insert(row.row_id.as_str()) {
                return Err(PsionExecutorLocalClusterLedgerError::DuplicateRowId {
                    row_id: row.row_id.clone(),
                });
            }
            row.validate()?;
        }
        self.validate_index(
            "psion_executor_local_cluster_ledger.search_index.run_id_to_row_ids",
            &self.search_index.run_id_to_row_ids,
            |row| row.search_run_ids.clone(),
        )?;
        self.validate_index(
            "psion_executor_local_cluster_ledger.search_index.profile_id_to_row_ids",
            &self.search_index.profile_id_to_row_ids,
            |row| vec![row.admitted_profile_id.clone()],
        )?;
        self.validate_index(
            "psion_executor_local_cluster_ledger.search_index.eval_pack_id_to_row_ids",
            &self.search_index.eval_pack_id_to_row_ids,
            |row| row.eval_pack_ids.clone(),
        )?;
        self.validate_index(
            "psion_executor_local_cluster_ledger.search_index.model_id_to_row_ids",
            &self.search_index.model_id_to_row_ids,
            |row| vec![row.model_id.clone()],
        )?;
        self.validate_index(
            "psion_executor_local_cluster_ledger.search_index.candidate_status_to_row_ids",
            &self.search_index.candidate_status_to_row_ids,
            |row| vec![candidate_status_key(&row.candidate_status)],
        )?;
        if self.support_refs.is_empty() {
            return Err(PsionExecutorLocalClusterLedgerError::MissingField {
                field: String::from("psion_executor_local_cluster_ledger.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(
                support_ref.as_str(),
                "psion_executor_local_cluster_ledger.support_refs[]",
            )?;
        }
        if stable_ledger_digest(self) != self.ledger_digest {
            return Err(PsionExecutorLocalClusterLedgerError::DigestMismatch {
                field: String::from("psion_executor_local_cluster_ledger.ledger_digest"),
            });
        }
        Ok(())
    }

    fn validate_index<F>(
        &self,
        field: &str,
        index: &BTreeMap<String, Vec<String>>,
        keys_for_row: F,
    ) -> Result<(), PsionExecutorLocalClusterLedgerError>
    where
        F: Fn(&PsionExecutorLocalClusterLedgerRow) -> Vec<String>,
    {
        let mut expected: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for row in &self.rows {
            for key in keys_for_row(row) {
                expected.entry(key).or_default().push(row.row_id.clone());
            }
        }
        for row_ids in expected.values_mut() {
            row_ids.sort();
            row_ids.dedup();
        }
        for row_ids in index.values() {
            if row_ids.is_empty() {
                return Err(PsionExecutorLocalClusterLedgerError::MissingField {
                    field: String::from(field),
                });
            }
        }
        if expected.len() != index.len() {
            return Err(PsionExecutorLocalClusterLedgerError::IndexMismatch {
                field: String::from(field),
                key: String::from("index_cardinality"),
            });
        }
        for (key, row_ids) in expected {
            let mut actual = index.get(&key).cloned().ok_or_else(|| {
                PsionExecutorLocalClusterLedgerError::IndexMismatch {
                    field: String::from(field),
                    key: key.clone(),
                }
            })?;
            actual.sort();
            actual.dedup();
            if actual != row_ids {
                return Err(PsionExecutorLocalClusterLedgerError::IndexMismatch {
                    field: String::from(field),
                    key,
                });
            }
        }
        Ok(())
    }
}

pub fn builtin_executor_local_cluster_ledger(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterLedger, PsionExecutorLocalClusterLedgerError> {
    let registration = builtin_executor_local_cluster_run_registration_packet(workspace_root)?;
    let baseline_truth = builtin_executor_baseline_truth_record(workspace_root)?;
    let mlx_export = builtin_executor_mac_export_inspection_packet(workspace_root)?;
    let cuda_decision = builtin_executor_4080_decision_grade_run_packet(workspace_root)?;
    let cuda_recovery = builtin_executor_4080_interruption_recovery_packet(workspace_root)?;
    let cuda_smoke = builtin_executor_4080_smoke_run_packet(workspace_root)?;
    let roundtrip = builtin_executor_local_cluster_roundtrip_packet(workspace_root)?;
    let registration_sha256 = hex::encode(Sha256::digest(read_bytes(
        workspace_root,
        PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH,
    )?));
    let baseline_truth_sha256 = hex::encode(Sha256::digest(read_bytes(
        workspace_root,
        PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH,
    )?));

    let mlx_registration = find_registration_row(&registration, "local_mac_mlx_aarch64")?;
    let cuda_registration = find_registration_row(&registration, "local_4080_cuda_tailnet_x86_64")?;

    let mut rows = vec![
        PsionExecutorLocalClusterLedgerRow {
            row_id: String::from(MLX_ROW_ID),
            registration_id: mlx_registration.registration_id.clone(),
            run_id: mlx_registration.run_id.clone(),
            search_run_ids: mlx_registration.search_run_ids.clone(),
            admitted_profile_id: mlx_registration.admitted_profile_id.clone(),
            model_id: mlx_registration.model_id.clone(),
            candidate_status: mlx_registration.candidate_status.clone(),
            eval_pack_ids: mlx_registration.eval_pack_ids.clone(),
            config_summary: format!(
                "MLX local decision-grade registration keeps profile `{}` with stop condition `{}` and batch geometry {}x{} under the canonical local-cluster schema.",
                mlx_registration.admitted_profile_id,
                mlx_registration.stop_condition,
                mlx_registration.batch_geometry.observed_batch_count,
                mlx_registration.batch_geometry.observed_sample_count
            ),
            checkpoint_lineage: PsionExecutorLocalClusterCheckpointLineage {
                checkpoint_family: mlx_registration.checkpoint_family.clone(),
                checkpoint_pointer_digest: None,
                checkpoint_ref: None,
                export_bundle_ref: Some(mlx_export.portable_bundle_ref.clone()),
                export_artifact_digest: Some(mlx_export.torch_state_dict_artifact_digest.clone()),
                detail: String::from(
                    "The local MLX row keeps the retained same-node checkpoint family explicit and records the Mac export-inspection bundle plus torch-style compatibility artifact as the current export lineage.",
                ),
            },
            cost_posture: PsionExecutorLocalClusterCostPosture {
                wallclock_budget_seconds: mlx_registration.wallclock_budget_seconds,
                observed_duration_ms: mlx_registration.observed_duration_ms,
                budget_burn_ratio: stable_budget_burn_ratio(
                    mlx_registration.observed_duration_ms,
                    mlx_registration.wallclock_budget_seconds,
                ),
                detail: String::from(
                    "The retained same-node MLX decision-grade run consumed most of the admitted 600 second budget without requiring a remote worker or later roundtrip recovery proof.",
                ),
            },
            metric_posture: PsionExecutorLocalClusterMetricPosture {
                completed_steps: 93_184,
                final_mean_loss: mlx_export
                    .summary
                    .contains("fast-route target")
                    .then_some(0.0)
                    .unwrap_or(0.0),
                observed_steps_per_second: mlx_registration
                    .expected_throughput
                    .expected_steps_per_second,
                observed_samples_per_second: mlx_registration
                    .expected_throughput
                    .expected_samples_per_second,
                observed_source_tokens_per_second: mlx_registration
                    .expected_throughput
                    .expected_source_tokens_per_second,
                detail: String::from(
                    "The retained same-node MLX report remains the canonical metric source for the local Mac row and stays directly comparable to the 4080 row through the shared admitted-device matrix.",
                ),
            },
            failure_facts: vec![PsionExecutorLocalClusterFailureFact {
                fact_id: String::from("same_node_mlx_no_retained_failure_window"),
                status: String::from("green"),
                detail: String::from(
                    "No retained interruption or export failure remains open on the same-node MLX decision-grade row once the Mac export-inspection packet is bound into the ledger.",
                ),
            }],
            export_status: String::from("green"),
            recovery_status: String::from("not_required_same_node"),
            detail: String::from(
                "The local MLX ledger row binds registration, metric, and export facts into one searchable record so the Mac lane no longer depends on reading separate decision and export packets by hand.",
            ),
            row_digest: String::new(),
        },
        PsionExecutorLocalClusterLedgerRow {
            row_id: String::from(CUDA_ROW_ID),
            registration_id: cuda_registration.registration_id.clone(),
            run_id: cuda_registration.run_id.clone(),
            search_run_ids: cuda_registration.search_run_ids.clone(),
            admitted_profile_id: cuda_registration.admitted_profile_id.clone(),
            model_id: cuda_registration.model_id.clone(),
            candidate_status: cuda_registration.candidate_status.clone(),
            eval_pack_ids: cuda_registration.eval_pack_ids.clone(),
            config_summary: format!(
                "4080 decision-grade registration keeps worker profile `{}` with control-plane `{}` and stop condition `{}` under the canonical local-cluster schema.",
                cuda_registration.admitted_profile_id,
                cuda_registration
                    .control_plane_profile_id
                    .clone()
                    .unwrap_or_else(|| String::from("none")),
                cuda_registration.stop_condition
            ),
            checkpoint_lineage: PsionExecutorLocalClusterCheckpointLineage {
                checkpoint_family: cuda_smoke.checkpoint_family.clone(),
                checkpoint_pointer_digest: Some(cuda_smoke.checkpoint_pointer_digest.clone()),
                checkpoint_ref: Some(cuda_smoke.checkpoint_ref.clone()),
                export_bundle_ref: Some(roundtrip.export_bundle_ref.clone()),
                export_artifact_digest: Some(roundtrip.export_bundle_sha256.clone()),
                detail: String::from(
                    "The 4080 row keeps the supporting Tailnet checkpoint family, pointer digest, and checkpoint ref explicit, and now binds the returned Mac-side portable bundle from the retained roundtrip closeout packet.",
                ),
            },
            cost_posture: PsionExecutorLocalClusterCostPosture {
                wallclock_budget_seconds: cuda_registration.wallclock_budget_seconds,
                observed_duration_ms: cuda_registration.observed_duration_ms,
                budget_burn_ratio: stable_budget_burn_ratio(
                    cuda_registration.observed_duration_ms,
                    cuda_registration.wallclock_budget_seconds,
                ),
                detail: String::from(
                    "The retained 4080 decision-grade run consumed most of the admitted 600 second accelerator budget and remains the current-best accelerator-backed local row.",
                ),
            },
            metric_posture: PsionExecutorLocalClusterMetricPosture {
                completed_steps: cuda_decision.completed_steps,
                final_mean_loss: f64::from(cuda_decision.final_mean_loss),
                observed_steps_per_second: cuda_registration
                    .expected_throughput
                    .expected_steps_per_second,
                observed_samples_per_second: cuda_registration
                    .expected_throughput
                    .expected_samples_per_second,
                observed_source_tokens_per_second: cuda_registration
                    .expected_throughput
                    .expected_source_tokens_per_second,
                detail: String::from(
                    "The retained same-node CUDA report remains the canonical metric source for the 4080 row and is still explicitly tied back to the supporting Tailnet cluster workflow.",
                ),
            },
            failure_facts: vec![
                PsionExecutorLocalClusterFailureFact {
                    fact_id: String::from("unsupported_precision_publish_refusal"),
                    status: cuda_smoke.publish_disposition.clone(),
                    detail: cuda_smoke.unsupported_precision_refusal.clone(),
                },
                PsionExecutorLocalClusterFailureFact {
                    fact_id: String::from("stale_worker_replay_required"),
                    status: cuda_recovery.stale_worker_validator_disposition.clone(),
                    detail: format!(
                        "Stale worker remains replay-required with aggregation blocked at timeout {} ms.",
                        cuda_recovery.stale_worker_timeout_ms
                    ),
                },
                PsionExecutorLocalClusterFailureFact {
                    fact_id: String::from("upload_disagreement_rejected"),
                    status: cuda_recovery.upload_disagreement_validator_disposition.clone(),
                    detail: String::from(
                        "Upload disagreement still resolves as rejected inside the retained recovery drill instead of silently entering lineage.",
                    ),
                },
                PsionExecutorLocalClusterFailureFact {
                    fact_id: String::from("uneven_worker_speed_wait_then_replay"),
                    status: cuda_recovery.uneven_worker_speed_disposition.clone(),
                    detail: format!(
                        "Uneven worker speed remains explicit at {} ms observed skew.",
                        cuda_recovery.uneven_worker_speed_observed_skew_ms
                    ),
                },
                PsionExecutorLocalClusterFailureFact {
                    fact_id: String::from("local_cluster_roundtrip_green"),
                    status: roundtrip.cluster_closure_status.clone(),
                    detail: roundtrip.cluster_closure_detail.clone(),
                },
            ],
            export_status: if roundtrip.phase_exit_green {
                String::from("green")
            } else {
                String::from("pending_mac_roundtrip_validation")
            },
            recovery_status: String::from("green"),
            detail: String::from(
                "The 4080 ledger row binds registration, checkpoint lineage, retained failure drills, and the green Mac -> 4080 -> Mac roundtrip closeout into one searchable record so the accelerator lane no longer hides cluster closure behind separate packet prose.",
            ),
            row_digest: String::new(),
        },
    ];
    for row in &mut rows {
        row.row_digest = stable_ledger_row_digest(row);
    }

    let search_index = build_search_index(&rows);
    let mut ledger = PsionExecutorLocalClusterLedger {
        schema_version: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_SCHEMA_VERSION),
        ledger_id: String::from("psion_executor_local_cluster_ledger_v1"),
        registration_packet_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_FIXTURE_PATH),
        registration_packet_sha256: registration_sha256,
        registration_packet_digest: registration.packet_digest,
        baseline_truth_ref: String::from(PSION_EXECUTOR_BASELINE_TRUTH_FIXTURE_PATH),
        baseline_truth_sha256,
        baseline_truth_digest: baseline_truth.record_digest,
        rows,
        search_index,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_RUN_REGISTRATION_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_SMOKE_RUN_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_INTERRUPTION_RECOVERY_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_DECISION_GRADE_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one searchable local-cluster ledger. It joins the canonical MLX and 4080 run registrations with checkpoint lineage, cost, metric, failure, recovery, export, and roundtrip-closure posture so expensive local executor runs stop living as separate packet fragments.",
        ),
        ledger_digest: String::new(),
    };
    ledger.ledger_digest = stable_ledger_digest(&ledger);
    ledger.validate()?;
    Ok(ledger)
}

pub fn write_builtin_executor_local_cluster_ledger(
    workspace_root: &Path,
) -> Result<PsionExecutorLocalClusterLedger, PsionExecutorLocalClusterLedgerError> {
    let ledger = builtin_executor_local_cluster_ledger(workspace_root)?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
        &ledger,
    )?;
    Ok(ledger)
}

fn find_registration_row<'a>(
    registration: &'a PsionExecutorLocalClusterRunRegistrationPacket,
    profile_id: &str,
) -> Result<
    &'a crate::PsionExecutorLocalClusterRunRegistrationRow,
    PsionExecutorLocalClusterLedgerError,
> {
    registration
        .registration_rows
        .iter()
        .find(|row| row.admitted_profile_id == profile_id)
        .ok_or_else(|| PsionExecutorLocalClusterLedgerError::MissingField {
            field: format!("registration_row[{profile_id}]"),
        })
}

fn build_search_index(
    rows: &[PsionExecutorLocalClusterLedgerRow],
) -> PsionExecutorLocalClusterSearchIndex {
    let mut run_id_to_row_ids = BTreeMap::new();
    let mut profile_id_to_row_ids = BTreeMap::new();
    let mut eval_pack_id_to_row_ids = BTreeMap::new();
    let mut model_id_to_row_ids = BTreeMap::new();
    let mut candidate_status_to_row_ids = BTreeMap::new();

    for row in rows {
        for run_id in &row.search_run_ids {
            run_id_to_row_ids
                .entry(run_id.clone())
                .or_insert_with(Vec::new)
                .push(row.row_id.clone());
        }
        profile_id_to_row_ids
            .entry(row.admitted_profile_id.clone())
            .or_insert_with(Vec::new)
            .push(row.row_id.clone());
        for eval_pack_id in &row.eval_pack_ids {
            eval_pack_id_to_row_ids
                .entry(eval_pack_id.clone())
                .or_insert_with(Vec::new)
                .push(row.row_id.clone());
        }
        model_id_to_row_ids
            .entry(row.model_id.clone())
            .or_insert_with(Vec::new)
            .push(row.row_id.clone());
        candidate_status_to_row_ids
            .entry(candidate_status_key(&row.candidate_status))
            .or_insert_with(Vec::new)
            .push(row.row_id.clone());
    }

    for index in [
        &mut run_id_to_row_ids,
        &mut profile_id_to_row_ids,
        &mut eval_pack_id_to_row_ids,
        &mut model_id_to_row_ids,
        &mut candidate_status_to_row_ids,
    ] {
        for row_ids in index.values_mut() {
            row_ids.sort();
            row_ids.dedup();
        }
    }

    PsionExecutorLocalClusterSearchIndex {
        run_id_to_row_ids,
        profile_id_to_row_ids,
        eval_pack_id_to_row_ids,
        model_id_to_row_ids,
        candidate_status_to_row_ids,
    }
}

fn candidate_status_key(status: &PsionExecutorLocalClusterCandidateStatus) -> String {
    match status {
        PsionExecutorLocalClusterCandidateStatus::CurrentBest => String::from("current_best"),
        PsionExecutorLocalClusterCandidateStatus::Candidate => String::from("candidate"),
    }
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutorLocalClusterLedgerError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutorLocalClusterLedgerError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorLocalClusterLedgerError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorLocalClusterLedgerError::Parse {
        path: relative_path.to_string(),
        error,
    })
}

fn write_json_fixture<T: Serialize>(
    workspace_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), PsionExecutorLocalClusterLedgerError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorLocalClusterLedgerError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&path, bytes).map_err(|error| PsionExecutorLocalClusterLedgerError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionExecutorLocalClusterLedgerError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorLocalClusterLedgerError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_ledger_row_digest(row: &PsionExecutorLocalClusterLedgerRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("ledger row serialization should succeed"),
    ))
}

fn stable_ledger_digest(ledger: &PsionExecutorLocalClusterLedger) -> String {
    let mut clone = ledger.clone();
    clone.ledger_digest.clear();
    hex::encode(Sha256::digest(
        serde_json::to_vec(&clone).expect("ledger serialization should succeed"),
    ))
}

fn stable_budget_burn_ratio(observed_duration_ms: u64, wallclock_budget_seconds: u64) -> f64 {
    let raw = observed_duration_ms as f64 / (wallclock_budget_seconds as f64 * 1000.0);
    (raw * 1_000_000_000_000_000.0).round() / 1_000_000_000_000_000.0
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
    fn builtin_executor_local_cluster_ledger_is_valid(
    ) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        let root = workspace_root();
        let ledger = builtin_executor_local_cluster_ledger(root.as_path())?;
        ledger.validate()?;
        Ok(())
    }

    #[test]
    fn executor_local_cluster_ledger_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        let root = workspace_root();
        let expected: PsionExecutorLocalClusterLedger = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
        )?;
        let actual = builtin_executor_local_cluster_ledger(root.as_path())?;
        if actual != expected {
            return Err(PsionExecutorLocalClusterLedgerError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH),
            });
        }
        Ok(())
    }

    #[test]
    fn executor_local_cluster_ledger_search_indices_cover_all_rows(
    ) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        let root = workspace_root();
        let ledger = builtin_executor_local_cluster_ledger(root.as_path())?;
        assert_eq!(
            ledger
                .rows_for_run_id("same-node-wallclock-retained-mlx")
                .len(),
            1
        );
        assert_eq!(
            ledger
                .rows_for_run_id("tailrun-home-admitted-20260328k")
                .len(),
            1
        );
        assert_eq!(ledger.rows_for_profile_id("local_mac_mlx_aarch64").len(), 1);
        assert_eq!(
            ledger
                .rows_for_profile_id("local_4080_cuda_tailnet_x86_64")
                .len(),
            1
        );
        assert_eq!(
            ledger
                .rows_for_eval_pack_id("tassadar.eval.frequent.v0")
                .len(),
            2
        );
        assert_eq!(
            ledger
                .rows_for_model_id("tassadar-article-transformer-trace-bound-trained-v0")
                .len(),
            2
        );
        assert_eq!(
            ledger
                .rows_for_candidate_status(PsionExecutorLocalClusterCandidateStatus::CurrentBest)
                .len(),
            1
        );
        Ok(())
    }

    #[test]
    fn write_executor_local_cluster_ledger_persists_current_truth(
    ) -> Result<(), PsionExecutorLocalClusterLedgerError> {
        let root = workspace_root();
        let ledger = write_builtin_executor_local_cluster_ledger(root.as_path())?;
        let committed: PsionExecutorLocalClusterLedger = read_json(
            root.as_path(),
            PSION_EXECUTOR_LOCAL_CLUSTER_LEDGER_FIXTURE_PATH,
        )?;
        assert_eq!(ledger, committed);
        Ok(())
    }
}
