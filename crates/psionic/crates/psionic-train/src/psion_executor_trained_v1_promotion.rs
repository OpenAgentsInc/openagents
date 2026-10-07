use std::{collections::BTreeSet, fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    builtin_executor_4080_decision_grade_run_packet,
    builtin_executor_decision_threshold_record,
    builtin_executor_local_cluster_roundtrip_packet,
    builtin_executor_long_run_rehearsal_packet,
    builtin_executor_mac_export_inspection_packet,
    builtin_executor_optimizer_ablation_packet,
    builtin_executor_percepta_closeout_status_packet,
    builtin_executor_supervision_density_ablation_packet,
    builtin_executor_tokenizer_architecture_gate_packet,
    builtin_executor_trace_family_weighting_ablation_packet,
    PsionExecutor4080DecisionGradeRunError, PsionExecutorDecisionThresholdError,
    PsionExecutorLocalClusterRoundtripError, PsionExecutorLongRunRehearsalError,
    PsionExecutorMacExportInspectionError, PsionExecutorOptimizerAblationError,
    PsionExecutorPerceptaCloseoutStatusError, PsionExecutorSupervisionDensityAblationError,
    PsionExecutorTokenizerArchitectureGateError, PsionExecutorTraceFamilyWeightingAblationError,
    PSION_EXECUTOR_4080_DECISION_GRADE_RUN_DOC_PATH,
    PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH,
    PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH,
    PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH,
    PSION_EXECUTOR_LONG_RUN_REHEARSAL_DOC_PATH, PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH,
    PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH, PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
    PSION_EXECUTOR_OPTIMIZER_ABLATION_DOC_PATH, PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
    PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_DOC_PATH,
    PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH,
    PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_DOC_PATH,
    PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
    PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_DOC_PATH,
    PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_FIXTURE_PATH,
    PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_DOC_PATH,
    PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
};

pub const PSION_EXECUTOR_TRAINED_V1_PROMOTION_SCHEMA_VERSION: &str =
    "psion.executor.trained_v1_promotion.v1";
pub const PSION_EXECUTOR_TRAINED_V1_PROMOTION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_trained_v1_promotion_v1.json";
pub const PSION_EXECUTOR_TRAINED_V1_PROMOTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_TRAINED_V1_PROMOTION.md";
pub const PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH: &str =
    "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v1_descriptor.json";
pub const PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH: &str =
    "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v1_artifact_manifest.json";
pub const PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH: &str =
    "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v1_lineage_contract.json";

const PACKET_ID: &str = "psion_executor_trained_v1_promotion_v1";
const DESCRIPTOR_SCHEMA_VERSION: &str = "psion.executor.trained_v1_descriptor.v1";
const DESCRIPTOR_ID: &str = "psion_executor_trained_v1_descriptor_v1";
const ARTIFACT_MANIFEST_SCHEMA_VERSION: &str = "psion.executor.trained_v1_artifact_manifest.v1";
const ARTIFACT_MANIFEST_ID: &str = "psion_executor_trained_v1_artifact_manifest_v1";
const LINEAGE_SCHEMA_VERSION: &str = "psion.executor.trained_v1_lineage_contract.v1";
const LINEAGE_CONTRACT_ID: &str = "psion_executor_trained_v1_lineage_contract_v1";
const BASE_MODEL_ID: &str = "tassadar-article-transformer-trace-bound-trained-v0";
const CANDIDATE_MODEL_ID: &str = "tassadar-article-transformer-trace-bound-trained-v1";
const CANDIDATE_ROUTE_ID: &str = "tassadar.article_route.direct_hull_cache_runtime.v1";
const MODEL_FAMILY: &str = "tassadar_article_transformer";
const ACCEPTANCE_PROFILE_REF: &str = "docs/PSION_EXECUTOR_ACCEPTANCE_PROFILE.md";
const ARTIFACT_NAMING_POLICY_REF: &str = "docs/PSION_EXECUTOR_ARTIFACT_NAMING.md";
const EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PARENT_LINEAGE_REF: &str =
    "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_lineage_contract.json";
const PARENT_LINEAGE_DIGEST: &str =
    "88973d5074f7d366e3e71d6cff5835c5e5d3ae3e338001906dec88538a18ad3e";

#[derive(Debug, Error)]
pub enum PsionExecutorTrainedV1PromotionError {
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
    #[error("schema version mismatch for `{field}`: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("digest mismatch for `{field}`")]
    DigestMismatch { field: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error("unknown optimizer threshold metric `{metric_id}`")]
    UnknownOptimizerMetric { metric_id: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    DecisionThreshold(#[from] PsionExecutorDecisionThresholdError),
    #[error(transparent)]
    Optimizer(#[from] PsionExecutorOptimizerAblationError),
    #[error(transparent)]
    TraceWeighting(#[from] PsionExecutorTraceFamilyWeightingAblationError),
    #[error(transparent)]
    SupervisionDensity(#[from] PsionExecutorSupervisionDensityAblationError),
    #[error(transparent)]
    TokenizerArchitecture(#[from] PsionExecutorTokenizerArchitectureGateError),
    #[error(transparent)]
    LongRun(#[from] PsionExecutorLongRunRehearsalError),
    #[error(transparent)]
    PerceptaCloseout(#[from] PsionExecutorPerceptaCloseoutStatusError),
    #[error(transparent)]
    MacExport(#[from] PsionExecutorMacExportInspectionError),
    #[error(transparent)]
    LocalClusterRoundtrip(#[from] PsionExecutorLocalClusterRoundtripError),
    #[error(transparent)]
    DecisionGrade4080(#[from] PsionExecutor4080DecisionGradeRunError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1Descriptor {
    pub schema_version: String,
    pub descriptor_id: String,
    pub model_id: String,
    pub family: String,
    pub revision: String,
    pub base_model_id: String,
    pub route_id: String,
    pub acceptance_profile_ref: String,
    pub artifact_naming_policy_ref: String,
    pub selected_receipt_issue_ids: Vec<String>,
    pub descriptor_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1ArtifactManifest {
    pub schema_version: String,
    pub manifest_id: String,
    pub model_id: String,
    pub artifact_id: String,
    pub artifact_format: String,
    pub primary_bundle_ref: String,
    pub primary_bundle_sha256: String,
    pub accelerator_bundle_ref: String,
    pub accelerator_bundle_sha256: String,
    pub replacement_publication_ref: String,
    pub replacement_publication_digest: String,
    pub remote_training_visibility_ref: String,
    pub remote_training_visibility_digest: String,
    pub local_cluster_roundtrip_ref: String,
    pub local_cluster_roundtrip_digest: String,
    pub manifest_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1LineageInputRow {
    pub issue_id: String,
    pub role: String,
    pub evidence_ref: String,
    pub evidence_digest: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1LineageContract {
    pub schema_version: String,
    pub contract_id: String,
    pub base_model_id: String,
    pub candidate_model_id: String,
    pub parent_lineage_ref: String,
    pub parent_lineage_digest: String,
    pub descriptor_ref: String,
    pub descriptor_digest: String,
    pub artifact_manifest_ref: String,
    pub artifact_manifest_digest: String,
    pub decision_threshold_ref: String,
    pub decision_threshold_digest: String,
    pub long_run_rehearsal_ref: String,
    pub long_run_rehearsal_digest: String,
    pub closeout_status_ref: String,
    pub closeout_status_digest: String,
    pub cluster_roundtrip_ref: String,
    pub cluster_roundtrip_digest: String,
    pub export_inspection_ref: String,
    pub export_inspection_digest: String,
    pub selected_input_rows: Vec<PsionExecutorTrainedV1LineageInputRow>,
    pub lineage_decision: String,
    pub claim_boundary: String,
    pub contract_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1GateRow {
    pub gate_id: String,
    pub evidence_ref: String,
    pub evidence_digest: String,
    pub status: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1PromotionInputRow {
    pub issue_id: String,
    pub packet_ref: String,
    pub packet_digest: String,
    pub role: String,
    pub detail: String,
    pub row_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorTrainedV1PromotionPacket {
    pub schema_version: String,
    pub packet_id: String,
    pub base_model_id: String,
    pub candidate_model_id: String,
    pub candidate_route_id: String,
    pub descriptor_ref: String,
    pub descriptor_digest: String,
    pub artifact_manifest_ref: String,
    pub artifact_manifest_digest: String,
    pub lineage_contract_ref: String,
    pub lineage_contract_digest: String,
    pub acceptance_profile_ref: String,
    pub artifact_naming_policy_ref: String,
    pub decision_threshold_ref: String,
    pub decision_threshold_digest: String,
    pub selected_input_rows: Vec<PsionExecutorTrainedV1PromotionInputRow>,
    pub gate_rows: Vec<PsionExecutorTrainedV1GateRow>,
    pub reference_linear_delta: f64,
    pub reference_linear_minimum_meaningful_delta: f64,
    pub hull_cache_delta: f64,
    pub hull_cache_minimum_meaningful_delta: f64,
    pub hull_cache_speedup_improvement: f64,
    pub hull_cache_speedup_floor: f64,
    pub cpu_gap_reduction: f64,
    pub cpu_gap_improvement_floor: f64,
    pub exactness_net_delta_bps: i32,
    pub held_out_delta_bps: i32,
    pub training_steps_per_second_delta: f64,
    pub saturated_gates_preserved: bool,
    pub non_saturated_threshold_cleared: bool,
    pub promotion_decision: String,
    pub support_refs: Vec<String>,
    pub summary: String,
    pub packet_digest: String,
}

impl PsionExecutorTrainedV1Descriptor {
    fn validate(&self) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_descriptor.schema_version",
                self.schema_version.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.descriptor_id",
                self.descriptor_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.family",
                self.family.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.revision",
                self.revision.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.base_model_id",
                self.base_model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.route_id",
                self.route_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.acceptance_profile_ref",
                self.acceptance_profile_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.artifact_naming_policy_ref",
                self.artifact_naming_policy_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_descriptor.descriptor_digest",
                self.descriptor_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.schema_version != DESCRIPTOR_SCHEMA_VERSION {
            return Err(PsionExecutorTrainedV1PromotionError::SchemaVersionMismatch {
                field: String::from("psion_executor_trained_v1_descriptor.schema_version"),
                expected: String::from(DESCRIPTOR_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        if self.model_id != CANDIDATE_MODEL_ID
            || self.base_model_id != BASE_MODEL_ID
            || self.route_id != CANDIDATE_ROUTE_ID
            || self.family != MODEL_FAMILY
            || self.revision != "v1"
            || self.acceptance_profile_ref != ACCEPTANCE_PROFILE_REF
            || self.artifact_naming_policy_ref != ARTIFACT_NAMING_POLICY_REF
        {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from("psion_executor_trained_v1_descriptor.identity"),
                detail: String::from(
                    "trained-v1 descriptor must keep the frozen phase-one executor naming and route identity",
                ),
            });
        }
        if self.selected_receipt_issue_ids != vec![
            String::from("#776"),
            String::from("#779"),
            String::from("#780"),
            String::from("#781"),
        ] {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_descriptor.selected_receipt_issue_ids",
                ),
                detail: String::from(
                    "trained-v1 descriptor must stay bound to the retained optimizer, trace-weighting, supervision, and architecture-gate receipts",
                ),
            });
        }
        if stable_descriptor_digest(self) != self.descriptor_digest {
            return Err(PsionExecutorTrainedV1PromotionError::DigestMismatch {
                field: String::from("psion_executor_trained_v1_descriptor.descriptor_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1ArtifactManifest {
    fn validate(&self) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_artifact_manifest.schema_version",
                self.schema_version.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.manifest_id",
                self.manifest_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.model_id",
                self.model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.artifact_id",
                self.artifact_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.artifact_format",
                self.artifact_format.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.primary_bundle_ref",
                self.primary_bundle_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.primary_bundle_sha256",
                self.primary_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.accelerator_bundle_ref",
                self.accelerator_bundle_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.accelerator_bundle_sha256",
                self.accelerator_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.replacement_publication_ref",
                self.replacement_publication_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.replacement_publication_digest",
                self.replacement_publication_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.remote_training_visibility_ref",
                self.remote_training_visibility_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.remote_training_visibility_digest",
                self.remote_training_visibility_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.local_cluster_roundtrip_ref",
                self.local_cluster_roundtrip_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.local_cluster_roundtrip_digest",
                self.local_cluster_roundtrip_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_artifact_manifest.manifest_digest",
                self.manifest_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.schema_version != ARTIFACT_MANIFEST_SCHEMA_VERSION
            || self.manifest_id != ARTIFACT_MANIFEST_ID
            || self.model_id != CANDIDATE_MODEL_ID
            || self.artifact_format != "portable_bundle_manifest"
        {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from("psion_executor_trained_v1_artifact_manifest.identity"),
                detail: String::from(
                    "trained-v1 artifact manifest must stay on the bounded executor portable-bundle contract",
                ),
            });
        }
        if stable_artifact_manifest_digest(self) != self.manifest_digest {
            return Err(PsionExecutorTrainedV1PromotionError::DigestMismatch {
                field: String::from("psion_executor_trained_v1_artifact_manifest.manifest_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1LineageInputRow {
    fn validate(&self) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_lineage_contract.selected_input_rows[].issue_id",
                self.issue_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.selected_input_rows[].role",
                self.role.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.selected_input_rows[].evidence_ref",
                self.evidence_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.selected_input_rows[].evidence_digest",
                self.evidence_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.selected_input_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.selected_input_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_lineage_input_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTrainedV1PromotionError::DigestMismatch {
                field: String::from(
                    "psion_executor_trained_v1_lineage_contract.selected_input_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1LineageContract {
    fn validate(
        &self,
        descriptor: &PsionExecutorTrainedV1Descriptor,
        artifact_manifest: &PsionExecutorTrainedV1ArtifactManifest,
    ) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_lineage_contract.schema_version",
                self.schema_version.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.contract_id",
                self.contract_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.base_model_id",
                self.base_model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.candidate_model_id",
                self.candidate_model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.parent_lineage_ref",
                self.parent_lineage_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.parent_lineage_digest",
                self.parent_lineage_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.descriptor_ref",
                self.descriptor_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.descriptor_digest",
                self.descriptor_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.artifact_manifest_ref",
                self.artifact_manifest_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.artifact_manifest_digest",
                self.artifact_manifest_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.decision_threshold_ref",
                self.decision_threshold_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.decision_threshold_digest",
                self.decision_threshold_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.long_run_rehearsal_ref",
                self.long_run_rehearsal_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.long_run_rehearsal_digest",
                self.long_run_rehearsal_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.closeout_status_ref",
                self.closeout_status_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.closeout_status_digest",
                self.closeout_status_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.cluster_roundtrip_ref",
                self.cluster_roundtrip_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.cluster_roundtrip_digest",
                self.cluster_roundtrip_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.export_inspection_ref",
                self.export_inspection_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.export_inspection_digest",
                self.export_inspection_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.lineage_decision",
                self.lineage_decision.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.claim_boundary",
                self.claim_boundary.as_str(),
            ),
            (
                "psion_executor_trained_v1_lineage_contract.contract_digest",
                self.contract_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.schema_version != LINEAGE_SCHEMA_VERSION
            || self.contract_id != LINEAGE_CONTRACT_ID
            || self.base_model_id != BASE_MODEL_ID
            || self.candidate_model_id != CANDIDATE_MODEL_ID
            || self.parent_lineage_ref != PARENT_LINEAGE_REF
            || self.parent_lineage_digest != PARENT_LINEAGE_DIGEST
            || self.descriptor_ref != PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH
            || self.artifact_manifest_ref != PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH
            || self.descriptor_digest != descriptor.descriptor_digest
            || self.artifact_manifest_digest != artifact_manifest.manifest_digest
        {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from("psion_executor_trained_v1_lineage_contract.identity"),
                detail: String::from(
                    "trained-v1 lineage must stay bound to the frozen v0 parent and the generated v1 descriptor/artifact surfaces",
                ),
            });
        }
        if self.selected_input_rows.len() != 4 {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from(
                    "psion_executor_trained_v1_lineage_contract.selected_input_rows",
                ),
                detail: String::from("trained-v1 lineage must cite four retained EPIC 8 input rows"),
            });
        }
        let mut seen = BTreeSet::new();
        for row in &self.selected_input_rows {
            row.validate()?;
            if !seen.insert(row.issue_id.as_str()) {
                return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                    field: String::from(
                        "psion_executor_trained_v1_lineage_contract.selected_input_rows[].issue_id",
                    ),
                    detail: String::from("trained-v1 lineage issue ids must be unique"),
                });
            }
        }
        if stable_lineage_contract_digest(self) != self.contract_digest {
            return Err(PsionExecutorTrainedV1PromotionError::DigestMismatch {
                field: String::from(
                    "psion_executor_trained_v1_lineage_contract.contract_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1GateRow {
    fn validate(&self) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_promotion.gate_rows[].gate_id",
                self.gate_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.gate_rows[].evidence_ref",
                self.evidence_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.gate_rows[].evidence_digest",
                self.evidence_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.gate_rows[].status",
                self.status.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.gate_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.gate_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.status != "green" {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from("psion_executor_trained_v1_promotion.gate_rows[].status"),
                detail: String::from("retained trained-v1 promotion gates must stay green"),
            });
        }
        if stable_gate_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTrainedV1PromotionError::DigestMismatch {
                field: String::from("psion_executor_trained_v1_promotion.gate_rows[].row_digest"),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1PromotionInputRow {
    fn validate(&self) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_promotion.selected_input_rows[].issue_id",
                self.issue_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.selected_input_rows[].packet_ref",
                self.packet_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.selected_input_rows[].packet_digest",
                self.packet_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.selected_input_rows[].role",
                self.role.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.selected_input_rows[].detail",
                self.detail.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.selected_input_rows[].row_digest",
                self.row_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if stable_promotion_input_row_digest(self) != self.row_digest {
            return Err(PsionExecutorTrainedV1PromotionError::DigestMismatch {
                field: String::from(
                    "psion_executor_trained_v1_promotion.selected_input_rows[].row_digest",
                ),
            });
        }
        Ok(())
    }
}

impl PsionExecutorTrainedV1PromotionPacket {
    pub fn validate(
        &self,
        descriptor: &PsionExecutorTrainedV1Descriptor,
        artifact_manifest: &PsionExecutorTrainedV1ArtifactManifest,
        lineage: &PsionExecutorTrainedV1LineageContract,
    ) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        for (field, value) in [
            (
                "psion_executor_trained_v1_promotion.schema_version",
                self.schema_version.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.base_model_id",
                self.base_model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.candidate_model_id",
                self.candidate_model_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.candidate_route_id",
                self.candidate_route_id.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.descriptor_ref",
                self.descriptor_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.descriptor_digest",
                self.descriptor_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.artifact_manifest_ref",
                self.artifact_manifest_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.artifact_manifest_digest",
                self.artifact_manifest_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.lineage_contract_ref",
                self.lineage_contract_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.lineage_contract_digest",
                self.lineage_contract_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.acceptance_profile_ref",
                self.acceptance_profile_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.artifact_naming_policy_ref",
                self.artifact_naming_policy_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.decision_threshold_ref",
                self.decision_threshold_ref.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.decision_threshold_digest",
                self.decision_threshold_digest.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.promotion_decision",
                self.promotion_decision.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.summary",
                self.summary.as_str(),
            ),
            (
                "psion_executor_trained_v1_promotion.packet_digest",
                self.packet_digest.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.schema_version != PSION_EXECUTOR_TRAINED_V1_PROMOTION_SCHEMA_VERSION
            || self.packet_id != PACKET_ID
            || self.base_model_id != BASE_MODEL_ID
            || self.candidate_model_id != CANDIDATE_MODEL_ID
            || self.candidate_route_id != CANDIDATE_ROUTE_ID
            || self.descriptor_ref != PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH
            || self.descriptor_digest != descriptor.descriptor_digest
            || self.artifact_manifest_ref != PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH
            || self.artifact_manifest_digest != artifact_manifest.manifest_digest
            || self.lineage_contract_ref != PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH
            || self.lineage_contract_digest != lineage.contract_digest
            || self.acceptance_profile_ref != ACCEPTANCE_PROFILE_REF
            || self.artifact_naming_policy_ref != ARTIFACT_NAMING_POLICY_REF
            || self.decision_threshold_ref != PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH
        {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from("psion_executor_trained_v1_promotion.identity"),
                detail: String::from(
                    "trained-v1 promotion packet must stay bound to the generated descriptor/artifact/lineage and the frozen acceptance surfaces",
                ),
            });
        }
        if self.selected_input_rows.len() != 4 {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from("psion_executor_trained_v1_promotion.selected_input_rows"),
                detail: String::from("trained-v1 promotion must keep four retained input rows"),
            });
        }
        let mut seen_issue_ids = BTreeSet::new();
        for row in &self.selected_input_rows {
            row.validate()?;
            if !seen_issue_ids.insert(row.issue_id.as_str()) {
                return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                    field: String::from(
                        "psion_executor_trained_v1_promotion.selected_input_rows[].issue_id",
                    ),
                    detail: String::from("trained-v1 promotion input rows must be unique"),
                });
            }
        }
        if self.gate_rows.len() != 10 {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from("psion_executor_trained_v1_promotion.gate_rows"),
                detail: String::from(
                    "trained-v1 promotion must keep the ten retained acceptance gates explicit",
                ),
            });
        }
        let mut seen_gate_ids = BTreeSet::new();
        for row in &self.gate_rows {
            row.validate()?;
            if !seen_gate_ids.insert(row.gate_id.as_str()) {
                return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                    field: String::from(
                        "psion_executor_trained_v1_promotion.gate_rows[].gate_id",
                    ),
                    detail: String::from("trained-v1 promotion gate ids must be unique"),
                });
            }
        }
        if !self.saturated_gates_preserved
            || !self.non_saturated_threshold_cleared
            || self.reference_linear_delta <= self.reference_linear_minimum_meaningful_delta
            || self.hull_cache_delta <= self.hull_cache_minimum_meaningful_delta
            || self.hull_cache_speedup_improvement <= self.hull_cache_speedup_floor
            || self.cpu_gap_reduction <= self.cpu_gap_improvement_floor
            || self.exactness_net_delta_bps < 0
            || self.held_out_delta_bps < 0
            || self.training_steps_per_second_delta <= 0.0
            || self.promotion_decision != "promote_trained_v1_candidate"
        {
            return Err(PsionExecutorTrainedV1PromotionError::InvalidValue {
                field: String::from("psion_executor_trained_v1_promotion.thresholds"),
                detail: String::from(
                    "trained-v1 promotion must preserve saturated gates and clear the frozen non-saturated thresholds",
                ),
            });
        }
        if stable_promotion_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorTrainedV1PromotionError::DigestMismatch {
                field: String::from("psion_executor_trained_v1_promotion.packet_digest"),
            });
        }
        Ok(())
    }
}

pub fn builtin_executor_trained_v1_descriptor() -> Result<PsionExecutorTrainedV1Descriptor, PsionExecutorTrainedV1PromotionError> {
    let mut descriptor = PsionExecutorTrainedV1Descriptor {
        schema_version: String::from(DESCRIPTOR_SCHEMA_VERSION),
        descriptor_id: String::from(DESCRIPTOR_ID),
        model_id: String::from(CANDIDATE_MODEL_ID),
        family: String::from(MODEL_FAMILY),
        revision: String::from("v1"),
        base_model_id: String::from(BASE_MODEL_ID),
        route_id: String::from(CANDIDATE_ROUTE_ID),
        acceptance_profile_ref: String::from(ACCEPTANCE_PROFILE_REF),
        artifact_naming_policy_ref: String::from(ARTIFACT_NAMING_POLICY_REF),
        selected_receipt_issue_ids: vec![
            String::from("#776"),
            String::from("#779"),
            String::from("#780"),
            String::from("#781"),
        ],
        descriptor_digest: String::new(),
    };
    descriptor.descriptor_digest = stable_descriptor_digest(&descriptor);
    descriptor.validate()?;
    Ok(descriptor)
}

pub fn builtin_executor_trained_v1_artifact_manifest(
    workspace_root: &Path,
) -> Result<PsionExecutorTrainedV1ArtifactManifest, PsionExecutorTrainedV1PromotionError> {
    let mac_export = builtin_executor_mac_export_inspection_packet(workspace_root)?;
    let decision_run = builtin_executor_4080_decision_grade_run_packet(workspace_root)?;
    let roundtrip = builtin_executor_local_cluster_roundtrip_packet(workspace_root)?;
    let mut manifest = PsionExecutorTrainedV1ArtifactManifest {
        schema_version: String::from(ARTIFACT_MANIFEST_SCHEMA_VERSION),
        manifest_id: String::from(ARTIFACT_MANIFEST_ID),
        model_id: String::from(CANDIDATE_MODEL_ID),
        artifact_id: String::from(
            "tassadar://article_transformer/weights/tassadar-article-transformer-trace-bound-trained-v1/promoted-v1",
        ),
        artifact_format: String::from("portable_bundle_manifest"),
        primary_bundle_ref: mac_export.portable_bundle_ref,
        primary_bundle_sha256: mac_export.portable_bundle_sha256,
        accelerator_bundle_ref: decision_run.retained_remote_bundle_ref,
        accelerator_bundle_sha256: decision_run.retained_remote_bundle_sha256,
        replacement_publication_ref: mac_export.replacement_publication_ref,
        replacement_publication_digest: mac_export.replacement_publication_digest,
        remote_training_visibility_ref: decision_run.visualization_bundle_ref,
        remote_training_visibility_digest: decision_run.visualization_bundle_digest,
        local_cluster_roundtrip_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH),
        local_cluster_roundtrip_digest: roundtrip.packet_digest,
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = stable_artifact_manifest_digest(&manifest);
    manifest.validate()?;
    Ok(manifest)
}

pub fn builtin_executor_trained_v1_lineage_contract(
    workspace_root: &Path,
) -> Result<PsionExecutorTrainedV1LineageContract, PsionExecutorTrainedV1PromotionError> {
    let descriptor = builtin_executor_trained_v1_descriptor()?;
    let manifest = builtin_executor_trained_v1_artifact_manifest(workspace_root)?;
    let optimizer = builtin_executor_optimizer_ablation_packet(workspace_root)?;
    let trace = builtin_executor_trace_family_weighting_ablation_packet(workspace_root)?;
    let supervision = builtin_executor_supervision_density_ablation_packet(workspace_root)?;
    let gate = builtin_executor_tokenizer_architecture_gate_packet(workspace_root)?;
    let thresholds = builtin_executor_decision_threshold_record(workspace_root)?;
    let long_run = builtin_executor_long_run_rehearsal_packet(workspace_root)?;
    let closeout = builtin_executor_percepta_closeout_status_packet(workspace_root)?;
    let roundtrip = builtin_executor_local_cluster_roundtrip_packet(workspace_root)?;
    let export = builtin_executor_mac_export_inspection_packet(workspace_root)?;

    let selected_input_rows = vec![
        build_lineage_input_row(
            "#776",
            "threshold_clearing_ablation",
            PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
            optimizer.packet_digest,
            "The optimizer ablation remains the retained threshold-clearing receipt for the first trained-v1 candidate.",
        ),
        build_lineage_input_row(
            "#779",
            "exactness_preservation_ablation",
            PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
            trace.packet_digest,
            "The trace-family weighting ablation remains the retained exactness-preservation and route-pressure receipt for the first trained-v1 candidate.",
        ),
        build_lineage_input_row(
            "#780",
            "stability_and_held_out_ablation",
            PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
            supervision.packet_digest,
            "The supervision-density ablation remains the retained stability and held-out cleanliness receipt for the first trained-v1 candidate.",
        ),
        build_lineage_input_row(
            "#781",
            "architecture_gate",
            PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_FIXTURE_PATH,
            gate.packet_digest,
            "The tokenizer/architecture gate remains the retained evidence that tokenizer work stayed blocked and the five-run same-baseline tranche is complete.",
        ),
    ];

    let mut contract = PsionExecutorTrainedV1LineageContract {
        schema_version: String::from(LINEAGE_SCHEMA_VERSION),
        contract_id: String::from(LINEAGE_CONTRACT_ID),
        base_model_id: String::from(BASE_MODEL_ID),
        candidate_model_id: String::from(CANDIDATE_MODEL_ID),
        parent_lineage_ref: String::from(PARENT_LINEAGE_REF),
        parent_lineage_digest: String::from(PARENT_LINEAGE_DIGEST),
        descriptor_ref: String::from(PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH),
        descriptor_digest: descriptor.descriptor_digest.clone(),
        artifact_manifest_ref: String::from(PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH),
        artifact_manifest_digest: manifest.manifest_digest.clone(),
        decision_threshold_ref: String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH),
        decision_threshold_digest: thresholds.record_digest,
        long_run_rehearsal_ref: String::from(PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH),
        long_run_rehearsal_digest: long_run.packet_digest,
        closeout_status_ref: String::from(PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_FIXTURE_PATH),
        closeout_status_digest: closeout.packet_digest,
        cluster_roundtrip_ref: String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH),
        cluster_roundtrip_digest: roundtrip.packet_digest,
        export_inspection_ref: String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH),
        export_inspection_digest: export.packet_digest,
        selected_input_rows,
        lineage_decision: String::from("derived_from_retained_epic8_candidate_receipts"),
        claim_boundary: String::from(
            "This lineage contract freezes only the first executor-capable Psion trained-v1 promotion candidate. It binds the retained threshold-clearing ablation, preserved-gate ablations, export path, roundtrip closure, and bounded closeout-green status without widening the executor workload family or weakening the reference-linear truth anchor.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = stable_lineage_contract_digest(&contract);
    contract.validate(&descriptor, &manifest)?;
    Ok(contract)
}

pub fn builtin_executor_trained_v1_promotion_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorTrainedV1PromotionPacket, PsionExecutorTrainedV1PromotionError> {
    let descriptor = builtin_executor_trained_v1_descriptor()?;
    let artifact_manifest = builtin_executor_trained_v1_artifact_manifest(workspace_root)?;
    let lineage = builtin_executor_trained_v1_lineage_contract(workspace_root)?;
    let optimizer = builtin_executor_optimizer_ablation_packet(workspace_root)?;
    let trace = builtin_executor_trace_family_weighting_ablation_packet(workspace_root)?;
    let supervision = builtin_executor_supervision_density_ablation_packet(workspace_root)?;
    let gate = builtin_executor_tokenizer_architecture_gate_packet(workspace_root)?;
    let long_run = builtin_executor_long_run_rehearsal_packet(workspace_root)?;
    let closeout = builtin_executor_percepta_closeout_status_packet(workspace_root)?;
    let export = builtin_executor_mac_export_inspection_packet(workspace_root)?;
    let roundtrip = builtin_executor_local_cluster_roundtrip_packet(workspace_root)?;
    let decision_run = builtin_executor_4080_decision_grade_run_packet(workspace_root)?;
    let thresholds = builtin_executor_decision_threshold_record(workspace_root)?;

    let reference_linear = optimizer_metric_row(
        &optimizer,
        "promotion_reference_linear_anchor_median_steps_per_second",
    )?;
    let hull_cache = optimizer_metric_row(
        &optimizer,
        "promotion_hull_cache_median_steps_per_second",
    )?;
    let hull_speedup = optimizer_metric_row(
        &optimizer,
        "promotion_hull_cache_min_speedup_over_reference_linear",
    )?;
    let cpu_gap = optimizer_metric_row(
        &optimizer,
        "promotion_hull_cache_max_remaining_gap_vs_cpu_reference",
    )?;

    let selected_input_rows = vec![
        build_promotion_input_row(
            "#776",
            PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
            optimizer.packet_digest.clone(),
            "threshold_clearing_ablation",
            "The optimizer ablation clears the retained non-saturated thresholds for reference-linear, hull-cache throughput, hull-cache speedup, and remaining CPU gap.",
        ),
        build_promotion_input_row(
            "#779",
            PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
            trace.packet_digest.clone(),
            "exactness_preservation_ablation",
            "The trace-family weighting ablation preserves exactness pressure while keeping held-out and adversarial regressions at zero.",
        ),
        build_promotion_input_row(
            "#780",
            PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
            supervision.packet_digest.clone(),
            "stability_and_held_out_ablation",
            "The supervision-density ablation keeps exactness, held-out, throughput, and stability green together on the admitted same-budget candidate lane.",
        ),
        build_promotion_input_row(
            "#781",
            PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_FIXTURE_PATH,
            gate.packet_digest.clone(),
            "evidence_gate",
            "The tokenizer and architecture gate records that tokenizer work stayed blocked and the five-run same-baseline tranche now exists for the trained-v1 promotion question.",
        ),
    ];

    let gate_rows = vec![
        build_gate_row(
            "exactness_gate_green",
            PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_FIXTURE_PATH,
            trace.packet_digest.clone(),
            "The retained trace-family weighting packet keeps exactness net-positive while preserving the already-green saturated exactness rows.",
        ),
        build_gate_row(
            "held_out_and_adversarial_gate_green",
            PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
            supervision.packet_digest.clone(),
            "Held-out stays net-positive on the retained supervision-density packet, while the retained trace-family packet keeps held-out and adversarial regressions at zero.",
        ),
        build_gate_row(
            "reference_linear_anchor_green",
            PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
            optimizer.packet_digest.clone(),
            "The retained optimizer packet clears the frozen reference-linear minimum meaningful delta and therefore keeps the measured baseline truth anchor green.",
        ),
        build_gate_row(
            "hull_cache_fast_route_green",
            PSION_EXECUTOR_OPTIMIZER_ABLATION_FIXTURE_PATH,
            optimizer.packet_digest.clone(),
            "The retained optimizer packet clears the frozen hull-cache throughput and speedup thresholds while preserving the admitted fast-route route id.",
        ),
        build_gate_row(
            "throughput_floor_green",
            PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_FIXTURE_PATH,
            supervision.packet_digest.clone(),
            "The retained supervision-density packet keeps throughput positive without trading away held-out or stability truth.",
        ),
        build_gate_row(
            "cpu_matrix_green",
            PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
            export.packet_digest.clone(),
            "The retained Mac export-inspection packet keeps CPU route validation green on host_cpu_aarch64 while preserving the wider reference-linear versus hull-cache claim boundary.",
        ),
        build_gate_row(
            "export_and_replacement_green",
            PSION_EXECUTOR_LONG_RUN_REHEARSAL_FIXTURE_PATH,
            long_run.packet_digest.clone(),
            "The retained long-run rehearsal packet keeps export candidate, replacement validation, and review-log truth green on the admitted executor lane.",
        ),
        build_gate_row(
            "local_cluster_green",
            PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_FIXTURE_PATH,
            roundtrip.packet_digest.clone(),
            "The retained Mac -> 4080 -> Mac roundtrip closeout packet keeps the local-cluster gate green for the candidate path.",
        ),
        build_gate_row(
            "remote_training_visibility_green",
            PSION_EXECUTOR_4080_DECISION_GRADE_RUN_FIXTURE_PATH,
            decision_run.packet_digest.clone(),
            "The retained 4080 decision-grade packet keeps remote-training visualization and run-index visibility green on the shipped operator surface.",
        ),
        build_gate_row(
            "promoted_artifact_compatibility_green",
            PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH,
            export.packet_digest.clone(),
            "The retained Mac export-inspection packet keeps replacement publication green, which is the current promoted-artifact compatibility seam for the bounded executor lane.",
        ),
    ];

    let mut packet = PsionExecutorTrainedV1PromotionPacket {
        schema_version: String::from(PSION_EXECUTOR_TRAINED_V1_PROMOTION_SCHEMA_VERSION),
        packet_id: String::from(PACKET_ID),
        base_model_id: String::from(BASE_MODEL_ID),
        candidate_model_id: String::from(CANDIDATE_MODEL_ID),
        candidate_route_id: String::from(CANDIDATE_ROUTE_ID),
        descriptor_ref: String::from(PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH),
        descriptor_digest: descriptor.descriptor_digest.clone(),
        artifact_manifest_ref: String::from(PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH),
        artifact_manifest_digest: artifact_manifest.manifest_digest.clone(),
        lineage_contract_ref: String::from(PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH),
        lineage_contract_digest: lineage.contract_digest.clone(),
        acceptance_profile_ref: String::from(ACCEPTANCE_PROFILE_REF),
        artifact_naming_policy_ref: String::from(ARTIFACT_NAMING_POLICY_REF),
        decision_threshold_ref: String::from(PSION_EXECUTOR_DECISION_THRESHOLDS_FIXTURE_PATH),
        decision_threshold_digest: thresholds.record_digest,
        selected_input_rows,
        gate_rows,
        reference_linear_delta: reference_linear.delta_value,
        reference_linear_minimum_meaningful_delta: reference_linear.minimum_meaningful_delta,
        hull_cache_delta: hull_cache.delta_value,
        hull_cache_minimum_meaningful_delta: hull_cache.minimum_meaningful_delta,
        hull_cache_speedup_improvement: hull_speedup.delta_value,
        hull_cache_speedup_floor: hull_speedup.minimum_meaningful_delta,
        cpu_gap_reduction: cpu_gap.delta_value,
        cpu_gap_improvement_floor: cpu_gap.minimum_meaningful_delta,
        exactness_net_delta_bps: trace.exactness_net_delta_bps,
        held_out_delta_bps: supervision.held_out_delta_bps,
        training_steps_per_second_delta: optimizer.training_steps_per_second_delta,
        saturated_gates_preserved: trace.held_out_negative_delta_count == 0
            && trace.adversarial_negative_delta_count == 0
            && supervision.stability_regression_count == 0
            && long_run.rehearsal_green
            && closeout.percepta_closeout_status == "green_bounded",
        non_saturated_threshold_cleared: true,
        promotion_decision: String::from("promote_trained_v1_candidate"),
        support_refs: vec![
            String::from(EXECUTOR_PROGRAM_DOC_PATH),
            String::from(ACCEPTANCE_PROFILE_REF),
            String::from(ARTIFACT_NAMING_POLICY_REF),
            String::from(PSION_EXECUTOR_OPTIMIZER_ABLATION_DOC_PATH),
            String::from(PSION_EXECUTOR_TRACE_FAMILY_WEIGHTING_ABLATION_DOC_PATH),
            String::from(PSION_EXECUTOR_SUPERVISION_DENSITY_ABLATION_DOC_PATH),
            String::from(PSION_EXECUTOR_TOKENIZER_ARCHITECTURE_GATE_DOC_PATH),
            String::from(PSION_EXECUTOR_LONG_RUN_REHEARSAL_DOC_PATH),
            String::from(PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS_DOC_PATH),
            String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH),
            String::from(PSION_EXECUTOR_LOCAL_CLUSTER_ROUNDTRIP_DOC_PATH),
            String::from(PSION_EXECUTOR_4080_DECISION_GRADE_RUN_DOC_PATH),
        ],
        summary: String::from(
            "The executor lane now has one retained trained-v1 promotion packet. The first candidate stays on the frozen tassadar artifact family, cites the retained optimizer, trace-weighting, supervision, and evidence-gate receipts directly, preserves all saturated gates, clears the retained non-saturated thresholds, and keeps export, local-cluster, CPU-matrix, and consumer-seam truth green on the admitted executor workload family.",
        ),
        packet_digest: String::new(),
    };
    packet.packet_digest = stable_promotion_packet_digest(&packet);
    packet.validate(&descriptor, &artifact_manifest, &lineage)?;
    Ok(packet)
}

pub fn write_builtin_executor_trained_v1_promotion_artifacts(
    workspace_root: &Path,
) -> Result<PsionExecutorTrainedV1PromotionPacket, PsionExecutorTrainedV1PromotionError> {
    let descriptor = builtin_executor_trained_v1_descriptor()?;
    let artifact_manifest = builtin_executor_trained_v1_artifact_manifest(workspace_root)?;
    let lineage = builtin_executor_trained_v1_lineage_contract(workspace_root)?;
    let packet = builtin_executor_trained_v1_promotion_packet(workspace_root)?;

    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH,
        &descriptor,
    )?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH,
        &artifact_manifest,
    )?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH,
        &lineage,
    )?;
    write_json_fixture(
        workspace_root,
        PSION_EXECUTOR_TRAINED_V1_PROMOTION_FIXTURE_PATH,
        &packet,
    )?;
    Ok(packet)
}

fn optimizer_metric_row<'a>(
    optimizer: &'a crate::PsionExecutorOptimizerAblationPacket,
    metric_id: &str,
) -> Result<&'a crate::PsionExecutorOptimizerAblationMetricRow, PsionExecutorTrainedV1PromotionError>
{
    optimizer
        .threshold_metric_rows
        .iter()
        .find(|row| row.metric_id == metric_id)
        .ok_or_else(|| PsionExecutorTrainedV1PromotionError::UnknownOptimizerMetric {
            metric_id: String::from(metric_id),
        })
}

fn build_lineage_input_row(
    issue_id: &str,
    role: &str,
    evidence_ref: &str,
    evidence_digest: String,
    detail: &str,
) -> PsionExecutorTrainedV1LineageInputRow {
    let mut row = PsionExecutorTrainedV1LineageInputRow {
        issue_id: String::from(issue_id),
        role: String::from(role),
        evidence_ref: String::from(evidence_ref),
        evidence_digest,
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_lineage_input_row_digest(&row);
    row
}

fn build_gate_row(
    gate_id: &str,
    evidence_ref: &str,
    evidence_digest: String,
    detail: &str,
) -> PsionExecutorTrainedV1GateRow {
    let mut row = PsionExecutorTrainedV1GateRow {
        gate_id: String::from(gate_id),
        evidence_ref: String::from(evidence_ref),
        evidence_digest,
        status: String::from("green"),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_gate_row_digest(&row);
    row
}

fn build_promotion_input_row(
    issue_id: &str,
    packet_ref: &str,
    packet_digest: String,
    role: &str,
    detail: &str,
) -> PsionExecutorTrainedV1PromotionInputRow {
    let mut row = PsionExecutorTrainedV1PromotionInputRow {
        issue_id: String::from(issue_id),
        packet_ref: String::from(packet_ref),
        packet_digest,
        role: String::from(role),
        detail: String::from(detail),
        row_digest: String::new(),
    };
    row.row_digest = stable_promotion_input_row_digest(&row);
    row
}

fn stable_descriptor_digest(descriptor: &PsionExecutorTrainedV1Descriptor) -> String {
    let mut clone = descriptor.clone();
    clone.descriptor_digest.clear();
    stable_json_digest("psion_executor_trained_v1_descriptor", &clone)
}

fn stable_artifact_manifest_digest(manifest: &PsionExecutorTrainedV1ArtifactManifest) -> String {
    let mut clone = manifest.clone();
    clone.manifest_digest.clear();
    stable_json_digest("psion_executor_trained_v1_artifact_manifest", &clone)
}

fn stable_lineage_input_row_digest(row: &PsionExecutorTrainedV1LineageInputRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_trained_v1_lineage_input_row", &clone)
}

fn stable_lineage_contract_digest(contract: &PsionExecutorTrainedV1LineageContract) -> String {
    let mut clone = contract.clone();
    clone.contract_digest.clear();
    stable_json_digest("psion_executor_trained_v1_lineage_contract", &clone)
}

fn stable_gate_row_digest(row: &PsionExecutorTrainedV1GateRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_trained_v1_gate_row", &clone)
}

fn stable_promotion_input_row_digest(row: &PsionExecutorTrainedV1PromotionInputRow) -> String {
    let mut clone = row.clone();
    clone.row_digest.clear();
    stable_json_digest("psion_executor_trained_v1_promotion_input_row", &clone)
}

fn stable_promotion_packet_digest(packet: &PsionExecutorTrainedV1PromotionPacket) -> String {
    let mut clone = packet.clone();
    clone.packet_digest.clear();
    stable_json_digest("psion_executor_trained_v1_promotion_packet", &clone)
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
) -> Result<(), PsionExecutorTrainedV1PromotionError> {
    let path = workspace_root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorTrainedV1PromotionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    fs::write(&path, body).map_err(|error| PsionExecutorTrainedV1PromotionError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorTrainedV1PromotionError> {
    let path = workspace_root.join(relative_path);
    let bytes = fs::read(&path).map_err(|error| PsionExecutorTrainedV1PromotionError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorTrainedV1PromotionError::Parse {
        path: path.display().to_string(),
        error,
    })
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorTrainedV1PromotionError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorTrainedV1PromotionError::MissingField {
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
    fn builtin_executor_trained_v1_promotion_packet_is_valid(
    ) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        let root = workspace_root();
        let descriptor = builtin_executor_trained_v1_descriptor()?;
        let manifest = builtin_executor_trained_v1_artifact_manifest(root.as_path())?;
        let lineage = builtin_executor_trained_v1_lineage_contract(root.as_path())?;
        let packet = builtin_executor_trained_v1_promotion_packet(root.as_path())?;
        descriptor.validate()?;
        manifest.validate()?;
        lineage.validate(&descriptor, &manifest)?;
        packet.validate(&descriptor, &manifest, &lineage)?;
        assert!(packet.saturated_gates_preserved);
        assert!(packet.non_saturated_threshold_cleared);
        assert_eq!(packet.promotion_decision, "promote_trained_v1_candidate");
        Ok(())
    }

    #[test]
    fn trained_v1_promotion_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorTrainedV1PromotionError> {
        let root = workspace_root();
        let expected_packet: PsionExecutorTrainedV1PromotionPacket = read_json(
            root.as_path(),
            PSION_EXECUTOR_TRAINED_V1_PROMOTION_FIXTURE_PATH,
        )?;
        let expected_descriptor: PsionExecutorTrainedV1Descriptor = read_json(
            root.as_path(),
            PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH,
        )?;
        let expected_manifest: PsionExecutorTrainedV1ArtifactManifest = read_json(
            root.as_path(),
            PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH,
        )?;
        let expected_lineage: PsionExecutorTrainedV1LineageContract = read_json(
            root.as_path(),
            PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH,
        )?;

        let actual_descriptor = builtin_executor_trained_v1_descriptor()?;
        let actual_manifest = builtin_executor_trained_v1_artifact_manifest(root.as_path())?;
        let actual_lineage = builtin_executor_trained_v1_lineage_contract(root.as_path())?;
        let actual_packet = builtin_executor_trained_v1_promotion_packet(root.as_path())?;

        if expected_descriptor.descriptor_digest != actual_descriptor.descriptor_digest {
            return Err(PsionExecutorTrainedV1PromotionError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_TRAINED_V1_DESCRIPTOR_PATH),
            });
        }
        if expected_manifest.manifest_digest != actual_manifest.manifest_digest {
            return Err(PsionExecutorTrainedV1PromotionError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_TRAINED_V1_ARTIFACT_MANIFEST_PATH),
            });
        }
        if expected_lineage.contract_digest != actual_lineage.contract_digest {
            return Err(PsionExecutorTrainedV1PromotionError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_TRAINED_V1_LINEAGE_CONTRACT_PATH),
            });
        }
        if expected_packet.packet_digest != actual_packet.packet_digest {
            return Err(PsionExecutorTrainedV1PromotionError::FixtureDrift {
                path: String::from(PSION_EXECUTOR_TRAINED_V1_PROMOTION_FIXTURE_PATH),
            });
        }
        Ok(())
    }
}
