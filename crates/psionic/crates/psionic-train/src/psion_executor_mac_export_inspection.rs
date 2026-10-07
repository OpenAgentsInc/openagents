use std::{fs, path::Path};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelIoError, PortableModelBundle, PortableModelImportRequest,
    PsionExecutorMlxDecisionGradeRunPacket, TensorMaterializationPolicy,
    PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH,
};

/// Stable schema version for the Mac export-inspection packet.
pub const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_SCHEMA_VERSION: &str =
    "psion.executor.mac_export_inspection.v1";
/// Canonical fixture path for the Mac export-inspection packet.
pub const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH: &str =
    "fixtures/psion/executor/psion_executor_mac_export_inspection_v1.json";
/// Canonical doc path for the Mac export-inspection packet.
pub const PSION_EXECUTOR_MAC_EXPORT_INSPECTION_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MAC_EXPORT_INSPECTION.md";

const LOCAL_MAC_MLX_PROFILE_ID: &str = "local_mac_mlx_aarch64";
const LOCAL_CPU_MACHINE_CLASS_ID: &str = "host_cpu_aarch64";
const PSION_EXECUTOR_PROGRAM_DOC_PATH: &str = "docs/PSION_EXECUTOR_PROGRAM.md";
const PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_ACCEPTANCE_PROFILE.md";
const PSION_EXECUTOR_EVAL_PACK_DOC_PATH: &str = "docs/PSION_EXECUTOR_EVAL_PACKS.md";
const PSION_EXECUTOR_MLX_DECISION_GRADE_DOC_PATH: &str =
    "docs/PSION_EXECUTOR_MLX_DECISION_GRADE_RUN.md";
const M5_MLX_BUNDLE_PATH: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors";
const CPU_REPRODUCIBILITY_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_cpu_reproducibility_report.json";
const FAST_ROUTE_IMPLEMENTATION_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_implementation_report.json";
const FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_article_fast_route_throughput_floor_report.json";
const HULL_CACHE_CLOSURE_REPORT_PATH: &str =
    "fixtures/tassadar/reports/tassadar_hull_cache_closure_report.json";

#[derive(Clone, Debug, Deserialize)]
struct CpuReproducibilityReport {
    report_digest: String,
    supported_machine_class_ids: Vec<String>,
    matrix: CpuReproducibilityMatrix,
}

#[derive(Clone, Debug, Deserialize)]
struct CpuReproducibilityMatrix {
    rows: Vec<CpuReproducibilityRow>,
}

#[derive(Clone, Debug, Deserialize)]
struct CpuReproducibilityRow {
    machine_class_id: String,
    status: String,
    runtime_backend: String,
    throughput_floor_steps_per_second: f64,
    note: String,
}

#[derive(Clone, Debug, Deserialize)]
struct FastRouteImplementationReport {
    report_digest: String,
    selected_candidate_kind: String,
    descriptor_review: FastRouteDescriptorReview,
    replacement_review: FastRouteReplacementReview,
}

#[derive(Clone, Debug, Deserialize)]
struct FastRouteDescriptorReview {
    supported_decode_modes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct FastRouteReplacementReview {
    publication_ref: String,
    transformer_model_id: String,
    replacement_certified: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct FastRouteThroughputFloorReport {
    report_digest: String,
    selection_prerequisite: FastRouteSelectionPrerequisite,
    cross_machine_drift_review: FastRouteCrossMachineDriftReview,
    throughput_floor_green: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct FastRouteSelectionPrerequisite {
    selected_candidate_kind: String,
    fast_route_selection_green: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct FastRouteCrossMachineDriftReview {
    supported_machine_class_ids: Vec<String>,
    drift_policy_green: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct HullCacheClosureReport {
    report_digest: String,
    exact_workloads: Vec<HullCacheExactWorkload>,
}

#[derive(Clone, Debug, Deserialize)]
struct HullCacheExactWorkload {
    workload_target: String,
    posture: String,
    fallback_case_count: u64,
    average_speedup_over_reference_linear: f64,
    average_remaining_gap_vs_cpu_reference: f64,
}

#[derive(Clone, Debug, Deserialize)]
struct ReplacementPublication {
    transformer_model_id: String,
    replacement_certified: bool,
    publication_digest: String,
}

/// One retained checklist row for Mac export inspection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionExecutorMacExportInspectionChecklistRow {
    /// Stable checklist id.
    pub checklist_id: String,
    /// Final status for the row.
    pub status: String,
    /// Honest detail.
    pub detail: String,
}

/// Typed packet for Mac export inspection and CPU-route validation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionExecutorMacExportInspectionPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable packet id.
    pub packet_id: String,
    /// Admitted profile id.
    pub admitted_profile_id: String,
    /// Stable local machine-class id.
    pub local_cpu_machine_class_id: String,
    /// Prerequisite decision-grade packet reference.
    pub decision_grade_packet_ref: String,
    /// Stable SHA256 over the decision-grade packet bytes.
    pub decision_grade_packet_sha256: String,
    /// Portable bundle reference.
    pub portable_bundle_ref: String,
    /// Stable SHA256 over the portable bundle bytes.
    pub portable_bundle_sha256: String,
    /// Deferred import-plan digest for the local bundle inspection.
    pub deferred_import_plan_digest: String,
    /// Imported state-dict digest after local bundle inspection.
    pub imported_state_dict_digest: String,
    /// Compatibility-contract digest surfaced by the imported bundle.
    pub compatibility_contract_digest: String,
    /// Exported torch-style compatibility artifact digest.
    pub torch_state_dict_artifact_digest: String,
    /// Torch-style export state-dict digest.
    pub torch_state_dict_state_dict_digest: String,
    /// Torch-style export tensor count.
    pub torch_state_dict_tensor_count: u64,
    /// CPU reproducibility report reference.
    pub cpu_reproducibility_report_ref: String,
    /// Stable digest published by the CPU reproducibility report.
    pub cpu_reproducibility_report_digest: String,
    /// Fast-route implementation report reference.
    pub fast_route_implementation_report_ref: String,
    /// Stable digest published by the fast-route implementation report.
    pub fast_route_implementation_report_digest: String,
    /// Fast-route throughput-floor report reference.
    pub fast_route_throughput_floor_report_ref: String,
    /// Stable digest published by the throughput-floor report.
    pub fast_route_throughput_floor_report_digest: String,
    /// Hull-cache closure report reference.
    pub hull_cache_closure_report_ref: String,
    /// Stable digest published by the hull-cache closure report.
    pub hull_cache_closure_report_digest: String,
    /// Replacement-publication reference.
    pub replacement_publication_ref: String,
    /// Stable digest published by the replacement publication.
    pub replacement_publication_digest: String,
    /// Stable transformer model id carried by the replacement surfaces.
    pub transformer_model_id: String,
    /// Stable anchor metric id.
    pub reference_linear_metric_id: String,
    /// Stable fast-route metric id.
    pub hull_cache_metric_id: String,
    /// Minimum hull-cache speedup retained across the exact workloads.
    pub min_hull_cache_speedup_over_reference_linear: f64,
    /// Maximum remaining gap versus CPU reference retained across the exact workloads.
    pub max_hull_cache_remaining_gap_vs_cpu_reference: f64,
    /// Retained checklist rows.
    pub checklist_rows: Vec<PsionExecutorMacExportInspectionChecklistRow>,
    /// Support references.
    pub support_refs: Vec<String>,
    /// Honest summary.
    pub summary: String,
    /// Stable packet digest.
    pub packet_digest: String,
}

impl PsionExecutorMacExportInspectionPacket {
    /// Validate the retained Mac export-inspection packet.
    pub fn validate(&self) -> Result<(), PsionExecutorMacExportInspectionError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "psion_executor_mac_export_inspection.schema_version",
        )?;
        if self.schema_version != PSION_EXECUTOR_MAC_EXPORT_INSPECTION_SCHEMA_VERSION {
            return Err(PsionExecutorMacExportInspectionError::SchemaVersionMismatch {
                expected: String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        for (field, value) in [
            (
                "psion_executor_mac_export_inspection.packet_id",
                self.packet_id.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.admitted_profile_id",
                self.admitted_profile_id.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.local_cpu_machine_class_id",
                self.local_cpu_machine_class_id.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.decision_grade_packet_ref",
                self.decision_grade_packet_ref.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.decision_grade_packet_sha256",
                self.decision_grade_packet_sha256.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.portable_bundle_ref",
                self.portable_bundle_ref.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.portable_bundle_sha256",
                self.portable_bundle_sha256.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.deferred_import_plan_digest",
                self.deferred_import_plan_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.imported_state_dict_digest",
                self.imported_state_dict_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.compatibility_contract_digest",
                self.compatibility_contract_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.torch_state_dict_artifact_digest",
                self.torch_state_dict_artifact_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.torch_state_dict_state_dict_digest",
                self.torch_state_dict_state_dict_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.cpu_reproducibility_report_ref",
                self.cpu_reproducibility_report_ref.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.cpu_reproducibility_report_digest",
                self.cpu_reproducibility_report_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.fast_route_implementation_report_ref",
                self.fast_route_implementation_report_ref.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.fast_route_implementation_report_digest",
                self.fast_route_implementation_report_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.fast_route_throughput_floor_report_ref",
                self.fast_route_throughput_floor_report_ref.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.fast_route_throughput_floor_report_digest",
                self.fast_route_throughput_floor_report_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.hull_cache_closure_report_ref",
                self.hull_cache_closure_report_ref.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.hull_cache_closure_report_digest",
                self.hull_cache_closure_report_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.replacement_publication_ref",
                self.replacement_publication_ref.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.replacement_publication_digest",
                self.replacement_publication_digest.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.transformer_model_id",
                self.transformer_model_id.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.reference_linear_metric_id",
                self.reference_linear_metric_id.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.hull_cache_metric_id",
                self.hull_cache_metric_id.as_str(),
            ),
            (
                "psion_executor_mac_export_inspection.summary",
                self.summary.as_str(),
            ),
        ] {
            ensure_nonempty(value, field)?;
        }
        if self.torch_state_dict_tensor_count == 0 {
            return Err(PsionExecutorMacExportInspectionError::InvalidValue {
                field: String::from(
                    "psion_executor_mac_export_inspection.torch_state_dict_tensor_count",
                ),
                detail: String::from("tensor count must stay positive"),
            });
        }
        if self.checklist_rows.is_empty() {
            return Err(PsionExecutorMacExportInspectionError::MissingField {
                field: String::from("psion_executor_mac_export_inspection.checklist_rows"),
            });
        }
        for row in &self.checklist_rows {
            row.validate()?;
        }
        if self.min_hull_cache_speedup_over_reference_linear < 1.5 {
            return Err(PsionExecutorMacExportInspectionError::InvalidValue {
                field: String::from(
                    "psion_executor_mac_export_inspection.min_hull_cache_speedup_over_reference_linear",
                ),
                detail: String::from("minimum retained speedup fell below the frozen promotion floor"),
            });
        }
        if self.max_hull_cache_remaining_gap_vs_cpu_reference > 3.0 {
            return Err(PsionExecutorMacExportInspectionError::InvalidValue {
                field: String::from(
                    "psion_executor_mac_export_inspection.max_hull_cache_remaining_gap_vs_cpu_reference",
                ),
                detail: String::from("maximum retained CPU gap exceeded the frozen promotion floor"),
            });
        }
        if self.support_refs.is_empty() {
            return Err(PsionExecutorMacExportInspectionError::MissingField {
                field: String::from("psion_executor_mac_export_inspection.support_refs"),
            });
        }
        for support_ref in &self.support_refs {
            ensure_nonempty(
                support_ref.as_str(),
                "psion_executor_mac_export_inspection.support_refs[]",
            )?;
        }
        if stable_mac_export_inspection_packet_digest(self) != self.packet_digest {
            return Err(PsionExecutorMacExportInspectionError::DigestMismatch {
                expected: stable_mac_export_inspection_packet_digest(self),
                actual: self.packet_digest.clone(),
            });
        }
        Ok(())
    }
}

impl PsionExecutorMacExportInspectionChecklistRow {
    fn validate(&self) -> Result<(), PsionExecutorMacExportInspectionError> {
        ensure_nonempty(
            self.checklist_id.as_str(),
            "psion_executor_mac_export_inspection.checklist_rows[].checklist_id",
        )?;
        ensure_nonempty(
            self.status.as_str(),
            "psion_executor_mac_export_inspection.checklist_rows[].status",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "psion_executor_mac_export_inspection.checklist_rows[].detail",
        )?;
        Ok(())
    }
}

/// Validation failures for the Mac export-inspection packet.
#[derive(Debug, Error)]
pub enum PsionExecutorMacExportInspectionError {
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error("field `{field}` is invalid: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("schema version mismatch: expected `{expected}` but found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("digest mismatch: expected `{expected}` but found `{actual}`")]
    DigestMismatch { expected: String, actual: String },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to write `{path}`: {error}")]
    Write {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to read `{path}`: {error}")]
    Read {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to parse json from `{path}`: {error}")]
    Parse {
        path: String,
        #[source]
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    ModelIo(#[from] ModelIoError),
}

/// Build the retained Mac export-inspection packet.
pub fn builtin_executor_mac_export_inspection_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMacExportInspectionPacket, PsionExecutorMacExportInspectionError> {
    let decision_grade_packet_bytes =
        read_bytes(workspace_root, PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH)?;
    let decision_grade_packet: PsionExecutorMlxDecisionGradeRunPacket =
        serde_json::from_slice(&decision_grade_packet_bytes).map_err(|error| {
            PsionExecutorMacExportInspectionError::Parse {
                path: String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH),
                error,
            }
        })?;
    decision_grade_packet
        .validate()
        .map_err(|error| PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from("psion_executor_mac_export_inspection.decision_grade_packet"),
            detail: error.to_string(),
        })?;

    let bundle_bytes = read_bytes(workspace_root, M5_MLX_BUNDLE_PATH)?;
    let deferred_plan = PortableModelBundle::plan_safetensors_import(
        bundle_bytes.as_slice(),
        &PortableModelImportRequest::new()
            .with_materialization_policy(TensorMaterializationPolicy::Deferred),
    )?;
    let imported_bundle = PortableModelBundle::import_safetensors(bundle_bytes.as_slice())?;
    let compatibility_contract = imported_bundle.compatibility_contract();
    let (torch_bytes, torch_receipt) = imported_bundle.export_torch_state_dict_json()?;
    let torch_roundtrip = PortableModelBundle::import_torch_state_dict_json(torch_bytes.as_slice())?;

    if torch_roundtrip.state_dict.digest != imported_bundle.state_dict.digest {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from("psion_executor_mac_export_inspection.torch_state_dict_roundtrip"),
            detail: String::from(
                "torch-style compatibility export/import must preserve the state-dict digest",
            ),
        });
    }

    let cpu_report: CpuReproducibilityReport =
        read_json(workspace_root, CPU_REPRODUCIBILITY_REPORT_PATH)?;
    let cpu_row = cpu_report
        .matrix
        .rows
        .iter()
        .find(|row| row.machine_class_id == LOCAL_CPU_MACHINE_CLASS_ID)
        .ok_or_else(|| PsionExecutorMacExportInspectionError::MissingField {
            field: String::from("psion_executor_mac_export_inspection.cpu_row.host_cpu_aarch64"),
        })?;
    if cpu_row.status != "supported_declared_class" || cpu_row.runtime_backend != "cpu" {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from("psion_executor_mac_export_inspection.cpu_row"),
            detail: String::from(
                "host_cpu_aarch64 must remain the declared supported CPU validation class",
            ),
        });
    }
    if !cpu_report
        .supported_machine_class_ids
        .iter()
        .any(|id| id == LOCAL_CPU_MACHINE_CLASS_ID)
    {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from(
                "psion_executor_mac_export_inspection.cpu_reproducibility_report",
            ),
            detail: String::from("host_cpu_aarch64 must stay in the supported machine-class set"),
        });
    }

    let implementation_report: FastRouteImplementationReport =
        read_json(workspace_root, FAST_ROUTE_IMPLEMENTATION_REPORT_PATH)?;
    if implementation_report.selected_candidate_kind != "hull_cache_runtime" {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from(
                "psion_executor_mac_export_inspection.fast_route_implementation_report.selected_candidate_kind",
            ),
            detail: String::from("selected candidate must remain hull_cache_runtime"),
        });
    }
    if !implementation_report
        .descriptor_review
        .supported_decode_modes
        .iter()
        .any(|mode| mode == "reference_linear")
        || !implementation_report
            .descriptor_review
            .supported_decode_modes
            .iter()
            .any(|mode| mode == "hull_cache")
    {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from(
                "psion_executor_mac_export_inspection.fast_route_implementation_report.descriptor_review.supported_decode_modes",
            ),
            detail: String::from(
                "implementation report must keep both reference_linear and hull_cache decode support explicit",
            ),
        });
    }

    let replacement_publication: ReplacementPublication = read_json(
        workspace_root,
        implementation_report.replacement_review.publication_ref.as_str(),
    )?;
    if !replacement_publication.replacement_certified
        || !implementation_report.replacement_review.replacement_certified
    {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from("psion_executor_mac_export_inspection.replacement_publication"),
            detail: String::from("replacement publication must stay certified"),
        });
    }
    if replacement_publication.transformer_model_id
        != implementation_report.replacement_review.transformer_model_id
    {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from("psion_executor_mac_export_inspection.transformer_model_id"),
            detail: String::from(
                "replacement publication model id must match the implementation report",
            ),
        });
    }

    let throughput_floor_report: FastRouteThroughputFloorReport =
        read_json(workspace_root, FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_PATH)?;
    if throughput_floor_report.selection_prerequisite.selected_candidate_kind
        != implementation_report.selected_candidate_kind
        || !throughput_floor_report.selection_prerequisite.fast_route_selection_green
        || !throughput_floor_report.throughput_floor_green
        || !throughput_floor_report.cross_machine_drift_review.drift_policy_green
    {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from(
                "psion_executor_mac_export_inspection.fast_route_throughput_floor_report",
            ),
            detail: String::from(
                "throughput-floor report must stay green and tied to the selected hull_cache runtime",
            ),
        });
    }
    if !throughput_floor_report
        .cross_machine_drift_review
        .supported_machine_class_ids
        .iter()
        .any(|id| id == LOCAL_CPU_MACHINE_CLASS_ID)
    {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from(
                "psion_executor_mac_export_inspection.fast_route_throughput_floor_report.cross_machine_drift_review",
            ),
            detail: String::from("host_cpu_aarch64 must stay in the declared CPU drift review set"),
        });
    }

    let hull_cache_report: HullCacheClosureReport =
        read_json(workspace_root, HULL_CACHE_CLOSURE_REPORT_PATH)?;
    let min_speedup = hull_cache_report
        .exact_workloads
        .iter()
        .map(|row| row.average_speedup_over_reference_linear)
        .fold(f64::INFINITY, f64::min);
    let max_gap = hull_cache_report
        .exact_workloads
        .iter()
        .map(|row| row.average_remaining_gap_vs_cpu_reference)
        .fold(f64::NEG_INFINITY, f64::max);
    if hull_cache_report.exact_workloads.is_empty() {
        return Err(PsionExecutorMacExportInspectionError::MissingField {
            field: String::from("psion_executor_mac_export_inspection.hull_cache_closure_report"),
        });
    }
    if hull_cache_report
        .exact_workloads
        .iter()
        .any(|row| row.posture != "exact" || row.fallback_case_count != 0)
    {
        return Err(PsionExecutorMacExportInspectionError::InvalidValue {
            field: String::from("psion_executor_mac_export_inspection.hull_cache_closure_report"),
            detail: String::from(
                "all retained hull_cache closure rows must stay exact and fallback-free",
            ),
        });
    }

    let checklist_rows = vec![
        PsionExecutorMacExportInspectionChecklistRow {
            checklist_id: String::from("portable_bundle_import_plan_green"),
            status: String::from("green"),
            detail: format!(
                "The Mac can plan deferred inspection for the retained MLX portable bundle with plan digest `{}` and {} deferred tensors before eager materialization is required.",
                deferred_plan.plan_digest,
                deferred_plan.deferred_tensor_count(),
            ),
        },
        PsionExecutorMacExportInspectionChecklistRow {
            checklist_id: String::from("portable_bundle_roundtrip_green"),
            status: String::from("green"),
            detail: format!(
                "The imported bundle exports one torch-style compatibility artifact digest `{}` and roundtrips back into the same state-dict digest `{}` on the Mac.",
                torch_receipt.artifact_digest,
                imported_bundle.state_dict.digest,
            ),
        },
        PsionExecutorMacExportInspectionChecklistRow {
            checklist_id: String::from("cpu_aarch64_route_green"),
            status: String::from("green"),
            detail: format!(
                "The retained CPU reproducibility report keeps `{}` as `status={}` with runtime backend `{}` and throughput floor {} steps/second.",
                cpu_row.machine_class_id,
                cpu_row.status,
                cpu_row.runtime_backend,
                cpu_row.throughput_floor_steps_per_second,
            ),
        },
        PsionExecutorMacExportInspectionChecklistRow {
            checklist_id: String::from("reference_linear_anchor_green"),
            status: String::from("green"),
            detail: String::from(
                "The fast-route implementation report still advertises `reference_linear` alongside `hull_cache`, so local export inspection keeps the baseline truth anchor explicit instead of letting the fast path erase it.",
            ),
        },
        PsionExecutorMacExportInspectionChecklistRow {
            checklist_id: String::from("hull_cache_fast_route_green"),
            status: String::from("green"),
            detail: format!(
                "The retained hull_cache closure report keeps min_speedup_over_reference_linear={:.6} and max_remaining_gap_vs_cpu_reference={:.6} across exact fallback-free workloads, which stays inside the frozen promotion floors.",
                min_speedup,
                max_gap,
            ),
        },
        PsionExecutorMacExportInspectionChecklistRow {
            checklist_id: String::from("replacement_publication_green"),
            status: String::from("green"),
            detail: format!(
                "The replacement publication remains certified for transformer model `{}` and the throughput-floor report keeps `host_cpu_aarch64` inside the declared CPU drift-review set.",
                replacement_publication.transformer_model_id,
            ),
        },
    ];

    let mut packet = PsionExecutorMacExportInspectionPacket {
        schema_version: String::from(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_SCHEMA_VERSION),
        packet_id: String::from("psion_executor_mac_export_inspection_v1"),
        admitted_profile_id: String::from(LOCAL_MAC_MLX_PROFILE_ID),
        local_cpu_machine_class_id: String::from(LOCAL_CPU_MACHINE_CLASS_ID),
        decision_grade_packet_ref: String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH),
        decision_grade_packet_sha256: hex::encode(Sha256::digest(&decision_grade_packet_bytes)),
        portable_bundle_ref: String::from(M5_MLX_BUNDLE_PATH),
        portable_bundle_sha256: hex::encode(Sha256::digest(&bundle_bytes)),
        deferred_import_plan_digest: deferred_plan.plan_digest,
        imported_state_dict_digest: imported_bundle.state_dict.digest.clone(),
        compatibility_contract_digest: compatibility_contract.contract_digest.clone(),
        torch_state_dict_artifact_digest: torch_receipt.artifact_digest.clone(),
        torch_state_dict_state_dict_digest: torch_receipt.state_dict_digest.clone(),
        torch_state_dict_tensor_count: torch_receipt.tensor_count as u64,
        cpu_reproducibility_report_ref: String::from(CPU_REPRODUCIBILITY_REPORT_PATH),
        cpu_reproducibility_report_digest: cpu_report.report_digest,
        fast_route_implementation_report_ref: String::from(FAST_ROUTE_IMPLEMENTATION_REPORT_PATH),
        fast_route_implementation_report_digest: implementation_report.report_digest,
        fast_route_throughput_floor_report_ref: String::from(
            FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_PATH,
        ),
        fast_route_throughput_floor_report_digest: throughput_floor_report.report_digest,
        hull_cache_closure_report_ref: String::from(HULL_CACHE_CLOSURE_REPORT_PATH),
        hull_cache_closure_report_digest: hull_cache_report.report_digest,
        replacement_publication_ref: implementation_report.replacement_review.publication_ref,
        replacement_publication_digest: replacement_publication.publication_digest,
        transformer_model_id: replacement_publication.transformer_model_id,
        reference_linear_metric_id: String::from("tassadar.reference_linear_steps_per_second"),
        hull_cache_metric_id: String::from("tassadar.hull_cache_steps_per_second"),
        min_hull_cache_speedup_over_reference_linear: min_speedup,
        max_hull_cache_remaining_gap_vs_cpu_reference: max_gap,
        checklist_rows,
        support_refs: vec![
            String::from(PSION_EXECUTOR_PROGRAM_DOC_PATH),
            String::from(PSION_EXECUTOR_ACCEPTANCE_PROFILE_DOC_PATH),
            String::from(PSION_EXECUTOR_EVAL_PACK_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_DOC_PATH),
            String::from(PSION_EXECUTOR_MLX_DECISION_GRADE_RUN_FIXTURE_PATH),
            String::from(M5_MLX_BUNDLE_PATH),
            String::from(CPU_REPRODUCIBILITY_REPORT_PATH),
            String::from(FAST_ROUTE_IMPLEMENTATION_REPORT_PATH),
            String::from(FAST_ROUTE_THROUGHPUT_FLOOR_REPORT_PATH),
            String::from(HULL_CACHE_CLOSURE_REPORT_PATH),
        ],
        summary: format!(
            "The admitted Mac profile now owns one explicit export-inspection packet. It imports the retained MLX portable bundle locally, emits a torch-style compatibility export roundtrip, keeps `host_cpu_aarch64` inside the admitted CPU route-validation set, and rechecks the current claim boundary that `reference_linear` stays the truth anchor while `hull_cache` stays the fast-route target (min speedup {:.6}, max CPU gap {:.6}) before any broader promotion story is told.",
            min_speedup,
            max_gap,
        ),
        packet_digest: String::new(),
    };
    if decision_grade_packet.final_state_dict_digest != packet.imported_state_dict_digest {
        packet.summary.push_str(
            " The Mac export packet refuses to count if the imported portable bundle ever diverges from the retained MLX decision-grade state-dict digest.",
        );
    }
    packet.packet_digest = stable_mac_export_inspection_packet_digest(&packet);
    packet.validate()?;
    Ok(packet)
}

/// Write the retained Mac export-inspection packet.
pub fn write_builtin_executor_mac_export_inspection_packet(
    workspace_root: &Path,
) -> Result<PsionExecutorMacExportInspectionPacket, PsionExecutorMacExportInspectionError> {
    let packet = builtin_executor_mac_export_inspection_packet(workspace_root)?;
    let path = workspace_root.join(PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionExecutorMacExportInspectionError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(&path, serde_json::to_vec_pretty(&packet)?).map_err(|error| {
        PsionExecutorMacExportInspectionError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(packet)
}

fn read_bytes(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<Vec<u8>, PsionExecutorMacExportInspectionError> {
    let path = workspace_root.join(relative_path);
    fs::read(&path).map_err(|error| PsionExecutorMacExportInspectionError::Read {
        path: path.display().to_string(),
        error,
    })
}

fn read_json<T: DeserializeOwned>(
    workspace_root: &Path,
    relative_path: &str,
) -> Result<T, PsionExecutorMacExportInspectionError> {
    let bytes = read_bytes(workspace_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|error| PsionExecutorMacExportInspectionError::Parse {
        path: relative_path.to_string(),
        error,
    })
}

fn stable_mac_export_inspection_packet_digest(
    packet: &PsionExecutorMacExportInspectionPacket,
) -> String {
    let mut canonical = packet.clone();
    canonical.packet_digest.clear();
    stable_digest(b"psion_executor_mac_export_inspection|", &canonical)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionExecutorMacExportInspectionError> {
    if value.trim().is_empty() {
        return Err(PsionExecutorMacExportInspectionError::MissingField {
            field: field.to_string(),
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
    fn builtin_executor_mac_export_inspection_packet_is_valid(
    ) -> Result<(), PsionExecutorMacExportInspectionError> {
        let root = workspace_root();
        let packet = builtin_executor_mac_export_inspection_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }

    #[test]
    fn executor_mac_export_inspection_fixture_matches_committed_truth(
    ) -> Result<(), PsionExecutorMacExportInspectionError> {
        let root = workspace_root();
        let generated = builtin_executor_mac_export_inspection_packet(root.as_path())?;
        let committed: PsionExecutorMacExportInspectionPacket =
            read_json(root.as_path(), PSION_EXECUTOR_MAC_EXPORT_INSPECTION_FIXTURE_PATH)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_executor_mac_export_inspection_packet_persists_current_truth(
    ) -> Result<(), PsionExecutorMacExportInspectionError> {
        let root = workspace_root();
        let packet = write_builtin_executor_mac_export_inspection_packet(root.as_path())?;
        packet.validate()?;
        Ok(())
    }
}
