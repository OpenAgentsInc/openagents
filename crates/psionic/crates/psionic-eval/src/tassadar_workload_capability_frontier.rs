use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{TassadarModuleScaleWorkloadFamily, TassadarModuleScaleWorkloadStatus};
use psionic_models::{
    tassadar_workload_hardness_taxonomy_publication, TassadarArchitectureBudgetDescriptor,
    TassadarFrontierLaneKind, TassadarSharedDepthWorkloadFamily, TassadarWorkloadHardnessClass,
    TassadarWorkloadHardnessTaxonomyPublication, TassadarWorkloadHardnessTaxonomyRow,
    TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF,
    TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_REPORT_REF,
};
use psionic_runtime::{
    TassadarRecurrentFastPathRuntimeBaselineReport, TassadarVerifierGuidedSearchWorkloadFamily,
    TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_shared_depth_halting_calibration_report, TassadarCapabilityPosture,
    TassadarClrsWasmBridgeReport, TassadarModuleScaleWorkloadSuiteReport,
    TassadarSharedDepthHaltingCalibrationReport, TassadarVerifierGuidedSearchEvaluationReport,
    TassadarWorkloadCapabilityMatrixReport, TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF,
    TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF, TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF,
    TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

/// How one preferred frontier lane currently lands for one workload family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkloadFrontierObservationPosture {
    /// The preferred lane has exact artifact-backed support for the family.
    Exact,
    /// The preferred lane remains exact only by an explicit fallback path.
    FallbackExact,
    /// The preferred lane exists as a research-only bounded surface.
    ResearchOnly,
    /// The preferred lane should currently refuse rather than silently degrade.
    RefuseFirst,
    /// No grounded artifact-backed observation exists for the preferred lane yet.
    UnderMapped,
}

/// One preferred-lane observation attached to a workload-family frontier row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadFrontierLaneObservation {
    /// Stable preferred lane kind.
    pub lane_kind: TassadarFrontierLaneKind,
    /// Current observation posture for the lane on this workload family.
    pub posture: TassadarWorkloadFrontierObservationPosture,
    /// Ordered artifact refs supporting the observation.
    pub artifact_refs: Vec<String>,
    /// Plain-language observation note.
    pub note: String,
}

/// One workload-family row in the capability frontier report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityFrontierRow {
    /// Stable workload family identifier.
    pub workload_family_id: String,
    /// Optional benchmark target carried by the taxonomy publication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_workload_target: Option<String>,
    /// Stable hardness classes for the family.
    pub hardness_classes: Vec<TassadarWorkloadHardnessClass>,
    /// Stable architecture-budget descriptor for the family.
    pub budget_descriptor: TassadarArchitectureBudgetDescriptor,
    /// Stable preferred lanes for the family.
    pub preferred_lanes: Vec<TassadarFrontierLaneKind>,
    /// Current observations for each preferred lane.
    pub observed_lanes: Vec<TassadarWorkloadFrontierLaneObservation>,
    /// Number of preferred lanes that remain under-mapped.
    pub under_mapped_lane_count: u32,
    /// Whether one preferred lane is still a refusal-first surface.
    pub refusal_first: bool,
    /// Row-level summary.
    pub summary: String,
}

/// Deterministic capability-frontier report over the current Tassadar workload families.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityFrontierReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Public taxonomy publication anchoring the report vocabulary.
    pub taxonomy_publication: TassadarWorkloadHardnessTaxonomyPublication,
    /// Ordered workload-family frontier rows.
    pub frontier_rows: Vec<TassadarWorkloadCapabilityFrontierRow>,
    /// Count of preferred-lane recommendations by lane label.
    pub preferred_lane_counts: BTreeMap<String, u32>,
    /// Workload families with at least one under-mapped preferred lane.
    pub under_mapped_workload_family_ids: Vec<String>,
    /// Workload families that still carry an explicit refusal-first preferred lane.
    pub refusal_first_workload_family_ids: Vec<String>,
    /// Ordered artifact refs used to build the report.
    pub generated_from_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Report summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarWorkloadCapabilityFrontierReport {
    fn new(
        taxonomy_publication: TassadarWorkloadHardnessTaxonomyPublication,
        frontier_rows: Vec<TassadarWorkloadCapabilityFrontierRow>,
        generated_from_refs: Vec<String>,
    ) -> Self {
        let preferred_lane_counts = count_preferred_lanes(&taxonomy_publication);
        let under_mapped_workload_family_ids = frontier_rows
            .iter()
            .filter(|row| row.under_mapped_lane_count > 0)
            .map(|row| row.workload_family_id.clone())
            .collect::<Vec<_>>();
        let refusal_first_workload_family_ids = frontier_rows
            .iter()
            .filter(|row| row.refusal_first)
            .map(|row| row.workload_family_id.clone())
            .collect::<Vec<_>>();
        let preferred_lane_total = frontier_rows
            .iter()
            .map(|row| row.observed_lanes.len() as u32)
            .sum::<u32>();
        let under_mapped_lane_total = frontier_rows
            .iter()
            .map(|row| row.under_mapped_lane_count)
            .sum::<u32>();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.workload_capability_frontier.report.v1"),
            taxonomy_publication,
            frontier_rows,
            preferred_lane_counts,
            under_mapped_workload_family_ids,
            refusal_first_workload_family_ids,
            generated_from_refs,
            claim_boundary: String::from(
                "this report is a research-only capability-frontier map over the current workload taxonomy and committed Tassadar artifacts. It keeps compiled exactness, recurrent runtime closure, verifier-guided search, and learned shared-depth architecture distinct, and it keeps under-mapped or refusal-first regions explicit instead of flattening them into one generic executor score",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Current workload frontier covers {} families and {} preferred-lane observations, with {} under-mapped preferred lanes across {} workload families and {} refusal-first workload families kept explicit.",
            report.frontier_rows.len(),
            preferred_lane_total,
            under_mapped_lane_total,
            report.under_mapped_workload_family_ids.len(),
            report.refusal_first_workload_family_ids.len(),
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_workload_capability_frontier_report|",
            &report,
        );
        report
    }
}

/// Frontier report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarWorkloadCapabilityFrontierReportError {
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read one committed artifact.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed workload capability frontier report.
pub fn build_tassadar_workload_capability_frontier_report(
) -> Result<TassadarWorkloadCapabilityFrontierReport, TassadarWorkloadCapabilityFrontierReportError>
{
    let taxonomy_publication = tassadar_workload_hardness_taxonomy_publication();
    let workload_matrix = load_repo_json::<TassadarWorkloadCapabilityMatrixReport>(
        TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF,
    )?;
    let module_scale = load_repo_json::<TassadarModuleScaleWorkloadSuiteReport>(
        TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
    )?;
    let clrs_bridge =
        load_repo_json::<TassadarClrsWasmBridgeReport>(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF)?;
    let search_report = load_repo_json::<TassadarVerifierGuidedSearchEvaluationReport>(
        TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF,
    )?;
    let recurrent_report = load_repo_json::<TassadarRecurrentFastPathRuntimeBaselineReport>(
        TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
    )?;
    let shared_depth_report = build_tassadar_shared_depth_halting_calibration_report();
    let frontier_rows = taxonomy_publication
        .rows
        .iter()
        .map(|row| {
            build_frontier_row(
                row,
                &workload_matrix,
                &module_scale,
                &clrs_bridge,
                &search_report,
                &recurrent_report,
                &shared_depth_report,
            )
        })
        .collect::<Vec<_>>();
    let generated_from_refs = generated_from_refs();

    Ok(TassadarWorkloadCapabilityFrontierReport::new(
        taxonomy_publication,
        frontier_rows,
        generated_from_refs,
    ))
}

/// Returns the canonical absolute path for the committed workload frontier report.
#[must_use]
pub fn tassadar_workload_capability_frontier_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_REPORT_REF)
}

/// Writes the committed workload capability frontier report.
pub fn write_tassadar_workload_capability_frontier_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWorkloadCapabilityFrontierReport, TassadarWorkloadCapabilityFrontierReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWorkloadCapabilityFrontierReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_workload_capability_frontier_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarWorkloadCapabilityFrontierReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_frontier_row(
    row: &TassadarWorkloadHardnessTaxonomyRow,
    workload_matrix: &TassadarWorkloadCapabilityMatrixReport,
    module_scale: &TassadarModuleScaleWorkloadSuiteReport,
    clrs_bridge: &TassadarClrsWasmBridgeReport,
    search_report: &TassadarVerifierGuidedSearchEvaluationReport,
    recurrent_report: &TassadarRecurrentFastPathRuntimeBaselineReport,
    shared_depth_report: &TassadarSharedDepthHaltingCalibrationReport,
) -> TassadarWorkloadCapabilityFrontierRow {
    let observed_lanes = row
        .preferred_lanes
        .iter()
        .map(|lane_kind| {
            observe_lane(
                row.workload_family_id.as_str(),
                *lane_kind,
                workload_matrix,
                module_scale,
                clrs_bridge,
                search_report,
                recurrent_report,
                shared_depth_report,
                row,
            )
        })
        .collect::<Vec<_>>();
    let under_mapped_lane_count = observed_lanes
        .iter()
        .filter(|observation| {
            observation.posture == TassadarWorkloadFrontierObservationPosture::UnderMapped
        })
        .count() as u32;
    let refusal_first = observed_lanes.iter().any(|observation| {
        observation.posture == TassadarWorkloadFrontierObservationPosture::RefuseFirst
    });
    let realized_lane_labels = observed_lanes
        .iter()
        .filter(|observation| {
            observation.posture != TassadarWorkloadFrontierObservationPosture::UnderMapped
        })
        .map(|observation| format!("{:?}", observation.lane_kind).to_lowercase())
        .collect::<Vec<_>>();

    TassadarWorkloadCapabilityFrontierRow {
        workload_family_id: row.workload_family_id.clone(),
        benchmark_workload_target: row.benchmark_workload_target.clone(),
        hardness_classes: row.hardness_classes.clone(),
        budget_descriptor: row.budget_descriptor.clone(),
        preferred_lanes: row.preferred_lanes.clone(),
        observed_lanes,
        under_mapped_lane_count,
        refusal_first,
        summary: format!(
            "frontier row `{}` keeps {} preferred lanes explicit, with {} currently grounded and {} under-mapped",
            row.workload_family_id,
            row.preferred_lanes.len(),
            realized_lane_labels.len(),
            under_mapped_lane_count,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn observe_lane(
    workload_family_id: &str,
    lane_kind: TassadarFrontierLaneKind,
    workload_matrix: &TassadarWorkloadCapabilityMatrixReport,
    module_scale: &TassadarModuleScaleWorkloadSuiteReport,
    clrs_bridge: &TassadarClrsWasmBridgeReport,
    search_report: &TassadarVerifierGuidedSearchEvaluationReport,
    recurrent_report: &TassadarRecurrentFastPathRuntimeBaselineReport,
    shared_depth_report: &TassadarSharedDepthHaltingCalibrationReport,
    taxonomy_row: &TassadarWorkloadHardnessTaxonomyRow,
) -> TassadarWorkloadFrontierLaneObservation {
    match lane_kind {
        TassadarFrontierLaneKind::CompiledExact => observe_compiled_exact_lane(
            workload_family_id,
            workload_matrix,
            module_scale,
            clrs_bridge,
        ),
        TassadarFrontierLaneKind::RuntimeRecurrentBaseline => {
            observe_recurrent_lane(workload_family_id, recurrent_report)
        }
        TassadarFrontierLaneKind::SearchSpecificTrace => {
            observe_search_lane(workload_family_id, search_report)
        }
        TassadarFrontierLaneKind::LearnedSharedDepth => {
            observe_shared_depth_lane(workload_family_id, shared_depth_report)
        }
        TassadarFrontierLaneKind::RefuseInsteadOfDegrade => {
            TassadarWorkloadFrontierLaneObservation {
                lane_kind,
                posture: TassadarWorkloadFrontierObservationPosture::RefuseFirst,
                artifact_refs: family_anchor_refs(workload_family_id),
                note: taxonomy_row.refusal_boundary.clone(),
            }
        }
    }
}

fn observe_compiled_exact_lane(
    workload_family_id: &str,
    workload_matrix: &TassadarWorkloadCapabilityMatrixReport,
    module_scale: &TassadarModuleScaleWorkloadSuiteReport,
    clrs_bridge: &TassadarClrsWasmBridgeReport,
) -> TassadarWorkloadFrontierLaneObservation {
    match workload_family_id {
        "long_loop_kernel" => compiled_exact_from_matrix(
            workload_matrix,
            &["long_loop_kernel"],
            "compiled exactness is grounded by the proof-backed long-loop kernel row in the workload matrix",
        ),
        "sudoku_class" => compiled_exact_from_matrix(
            workload_matrix,
            &["sudoku_class_4x4", "sudoku_search_9x9"],
            "compiled exactness is grounded by the 4x4 and 9x9 Sudoku compiled rows in the workload matrix",
        ),
        "hungarian_matching" => compiled_exact_from_matrix(
            workload_matrix,
            &["hungarian_matching_4x4", "hungarian_matching_10x10"],
            "compiled exactness is grounded by the 4x4 and 10x10 Hungarian compiled rows in the workload matrix",
        ),
        "clrs_shortest_path" => {
            let all_exact = clrs_bridge
                .length_generalization_matrix
                .iter()
                .all(|cell| cell.exactness_bps == 10_000);
            TassadarWorkloadFrontierLaneObservation {
                lane_kind: TassadarFrontierLaneKind::CompiledExact,
                posture: if all_exact {
                    TassadarWorkloadFrontierObservationPosture::Exact
                } else {
                    TassadarWorkloadFrontierObservationPosture::UnderMapped
                },
                artifact_refs: vec![String::from(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF)],
                note: format!(
                    "CLRS shortest-path bridge keeps sequential and wavefront trajectory families explicit across {} comparison rows",
                    clrs_bridge.trajectory_comparisons.len()
                ),
            }
        }
        "module_memcpy" => compiled_exact_from_module_scale(
            module_scale,
            TassadarModuleScaleWorkloadFamily::Memcpy,
            "module-scale memcpy stays exact under the current bounded lowering lane",
        ),
        "module_parsing" => compiled_exact_from_module_scale(
            module_scale,
            TassadarModuleScaleWorkloadFamily::Parsing,
            "module-scale parsing stays exact under the current bounded lowering lane",
        ),
        "module_checksum" => compiled_exact_from_module_scale(
            module_scale,
            TassadarModuleScaleWorkloadFamily::Checksum,
            "module-scale checksum stays exact under the current bounded lowering lane",
        ),
        "module_vm_style" => compiled_exact_from_module_scale(
            module_scale,
            TassadarModuleScaleWorkloadFamily::VmStyle,
            "module-scale VM-style dispatch stays exact under the current bounded lowering lane",
        ),
        _ => TassadarWorkloadFrontierLaneObservation {
            lane_kind: TassadarFrontierLaneKind::CompiledExact,
            posture: TassadarWorkloadFrontierObservationPosture::UnderMapped,
            artifact_refs: vec![String::from(TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF)],
            note: String::from(
                "no proof-backed compiled exact row is currently published for this family; the frontier keeps that absence explicit",
            ),
        },
    }
}

fn compiled_exact_from_matrix(
    workload_matrix: &TassadarWorkloadCapabilityMatrixReport,
    row_ids: &[&str],
    note: &str,
) -> TassadarWorkloadFrontierLaneObservation {
    let exact_compiled_refs = row_ids
        .iter()
        .filter_map(|row_id| {
            workload_matrix
                .rows
                .iter()
                .find(|row| row.workload_family_id == *row_id)
        })
        .flat_map(|row| row.capabilities.iter())
        .filter(|cell| {
            cell.surface_id == "compiled.proof_backed"
                && cell.posture == TassadarCapabilityPosture::Exact
        })
        .map(|cell| cell.artifact_ref.clone())
        .collect::<Vec<_>>();

    TassadarWorkloadFrontierLaneObservation {
        lane_kind: TassadarFrontierLaneKind::CompiledExact,
        posture: if exact_compiled_refs.is_empty() {
            TassadarWorkloadFrontierObservationPosture::UnderMapped
        } else {
            TassadarWorkloadFrontierObservationPosture::Exact
        },
        artifact_refs: if exact_compiled_refs.is_empty() {
            vec![String::from(TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF)]
        } else {
            exact_compiled_refs
        },
        note: String::from(note),
    }
}

fn compiled_exact_from_module_scale(
    module_scale: &TassadarModuleScaleWorkloadSuiteReport,
    family: TassadarModuleScaleWorkloadFamily,
    note: &str,
) -> TassadarWorkloadFrontierLaneObservation {
    let family_cases = module_scale
        .cases
        .iter()
        .filter(|case| case.family == family)
        .collect::<Vec<_>>();
    let all_exact = !family_cases.is_empty()
        && family_cases
            .iter()
            .all(|case| case.status == TassadarModuleScaleWorkloadStatus::LoweredExact);

    TassadarWorkloadFrontierLaneObservation {
        lane_kind: TassadarFrontierLaneKind::CompiledExact,
        posture: if all_exact {
            TassadarWorkloadFrontierObservationPosture::Exact
        } else {
            TassadarWorkloadFrontierObservationPosture::UnderMapped
        },
        artifact_refs: vec![String::from(
            TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
        )],
        note: format!("{note}; exact_case_count={}", family_cases.len()),
    }
}

fn observe_recurrent_lane(
    workload_family_id: &str,
    recurrent_report: &TassadarRecurrentFastPathRuntimeBaselineReport,
) -> TassadarWorkloadFrontierLaneObservation {
    if recurrent_report
        .direct_workload_families
        .iter()
        .any(|family| family == workload_family_id)
    {
        return TassadarWorkloadFrontierLaneObservation {
            lane_kind: TassadarFrontierLaneKind::RuntimeRecurrentBaseline,
            posture: TassadarWorkloadFrontierObservationPosture::Exact,
            artifact_refs: vec![String::from(
                TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
            )],
            note: String::from(
                "the artifact-backed recurrent fast-path runtime currently executes this family directly and exactly",
            ),
        };
    }

    if recurrent_report
        .fallback_workload_families
        .iter()
        .any(|family| family == workload_family_id)
    {
        return TassadarWorkloadFrontierLaneObservation {
            lane_kind: TassadarFrontierLaneKind::RuntimeRecurrentBaseline,
            posture: TassadarWorkloadFrontierObservationPosture::FallbackExact,
            artifact_refs: vec![String::from(
                TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
            )],
            note: String::from(
                "the artifact-backed recurrent fast-path runtime currently reaches this family only through explicit exact fallback",
            ),
        };
    }

    TassadarWorkloadFrontierLaneObservation {
        lane_kind: TassadarFrontierLaneKind::RuntimeRecurrentBaseline,
        posture: TassadarWorkloadFrontierObservationPosture::UnderMapped,
        artifact_refs: vec![String::from(
            TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
        )],
        note: String::from(
            "the current recurrent runtime baseline does not publish this family in either its direct or exact-fallback sets",
        ),
    }
}

fn observe_search_lane(
    workload_family_id: &str,
    search_report: &TassadarVerifierGuidedSearchEvaluationReport,
) -> TassadarWorkloadFrontierLaneObservation {
    if workload_family_id == "sudoku_class" {
        let summary = search_report.family_summaries.iter().find(|summary| {
            summary.workload_family
                == TassadarVerifierGuidedSearchWorkloadFamily::SudokuBacktracking
        });
        if let Some(summary) = summary {
            return TassadarWorkloadFrontierLaneObservation {
                lane_kind: TassadarFrontierLaneKind::SearchSpecificTrace,
                posture: TassadarWorkloadFrontierObservationPosture::ResearchOnly,
                artifact_refs: vec![String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF)],
                note: format!(
                    "research-only verifier-guided search currently anchors Sudoku with {} seeded cases, verifier_certificate_accuracy_bps={}, and recovery_quality_bps={}",
                    summary.case_count,
                    summary.verifier_certificate_accuracy_bps,
                    summary.recovery_quality_bps,
                ),
            };
        }
    }

    TassadarWorkloadFrontierLaneObservation {
        lane_kind: TassadarFrontierLaneKind::SearchSpecificTrace,
        posture: TassadarWorkloadFrontierObservationPosture::UnderMapped,
        artifact_refs: vec![String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF)],
        note: String::from(
            "no verifier-guided search trace family is currently published for this workload family",
        ),
    }
}

fn observe_shared_depth_lane(
    workload_family_id: &str,
    shared_depth_report: &TassadarSharedDepthHaltingCalibrationReport,
) -> TassadarWorkloadFrontierLaneObservation {
    let family = match workload_family_id {
        "long_loop_kernel" => Some(TassadarSharedDepthWorkloadFamily::LoopHeavyKernel),
        "module_vm_style" => Some(TassadarSharedDepthWorkloadFamily::CallHeavyModule),
        _ => None,
    };

    let Some(family) = family else {
        return TassadarWorkloadFrontierLaneObservation {
            lane_kind: TassadarFrontierLaneKind::LearnedSharedDepth,
            posture: TassadarWorkloadFrontierObservationPosture::UnderMapped,
            artifact_refs: vec![String::from(TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF)],
            note: String::from(
                "the current shared-depth lane only publishes loop-heavy-kernel and call-heavy-module families",
            ),
        };
    };

    let dynamic_row = shared_depth_report
        .rows
        .iter()
        .find(|row| {
            row.workload_family == family
                && row.variant_id.as_str() == "shared_depth_dynamic_halting"
        })
        .expect("shared-depth dynamic row should exist for published families");

    TassadarWorkloadFrontierLaneObservation {
        lane_kind: TassadarFrontierLaneKind::LearnedSharedDepth,
        posture: TassadarWorkloadFrontierObservationPosture::ResearchOnly,
        artifact_refs: vec![String::from(TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF)],
        note: format!(
            "research-only shared-depth dynamic halting currently reports later_window_exactness_bps={} and budget_exhaustion_rate_bps={} for this family",
            dynamic_row.later_window_exactness_bps,
            dynamic_row.budget_exhaustion_rate_bps,
        ),
    }
}

fn generated_from_refs() -> Vec<String> {
    let mut refs = BTreeSet::new();
    refs.insert(String::from(TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF));
    refs.insert(String::from(
        TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
    ));
    refs.insert(String::from(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF));
    refs.insert(String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF));
    refs.insert(String::from(
        TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF,
    ));
    refs.insert(String::from(TASSADAR_SHARED_DEPTH_ARCHITECTURE_REPORT_REF));
    refs.into_iter().collect()
}

fn family_anchor_refs(workload_family_id: &str) -> Vec<String> {
    match workload_family_id {
        "clrs_shortest_path" => vec![String::from(TASSADAR_CLRS_WASM_BRIDGE_REPORT_REF)],
        "sudoku_class" | "hungarian_matching" => vec![
            String::from(TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF),
            String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF),
        ],
        "module_memcpy" | "module_parsing" | "module_checksum" | "module_vm_style" => {
            vec![String::from(
                TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF,
            )]
        }
        _ => vec![
            String::from(TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF),
            String::from(TASSADAR_RECURRENT_FAST_PATH_RUNTIME_BASELINE_REPORT_REF),
        ],
    }
}

fn count_preferred_lanes(
    publication: &TassadarWorkloadHardnessTaxonomyPublication,
) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for row in &publication.rows {
        for lane in &row.preferred_lanes {
            *counts
                .entry(frontier_lane_label(*lane).to_string())
                .or_insert(0) += 1;
        }
    }
    counts
}

fn frontier_lane_label(lane: TassadarFrontierLaneKind) -> &'static str {
    match lane {
        TassadarFrontierLaneKind::CompiledExact => "compiled_exact",
        TassadarFrontierLaneKind::RuntimeRecurrentBaseline => "runtime_recurrent_baseline",
        TassadarFrontierLaneKind::SearchSpecificTrace => "search_specific_trace",
        TassadarFrontierLaneKind::LearnedSharedDepth => "learned_shared_depth",
        TassadarFrontierLaneKind::RefuseInsteadOfDegrade => "refuse_instead_of_degrade",
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn load_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarWorkloadCapabilityFrontierReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarWorkloadCapabilityFrontierReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarWorkloadCapabilityFrontierReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_workload_capability_frontier_report, load_repo_json,
        tassadar_workload_capability_frontier_report_path,
        write_tassadar_workload_capability_frontier_report,
        TassadarWorkloadCapabilityFrontierReport, TassadarWorkloadFrontierObservationPosture,
    };
    use psionic_models::TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_REPORT_REF;

    #[test]
    fn workload_capability_frontier_report_maps_current_frontier_boundaries() {
        let report = build_tassadar_workload_capability_frontier_report().expect("report");

        let micro = report
            .frontier_rows
            .iter()
            .find(|row| row.workload_family_id == "micro_wasm_kernel")
            .expect("micro row");
        assert!(micro.under_mapped_lane_count > 0);

        let long_loop = report
            .frontier_rows
            .iter()
            .find(|row| row.workload_family_id == "long_loop_kernel")
            .expect("long loop row");
        assert!(long_loop.observed_lanes.iter().any(|observation| {
            observation.posture == TassadarWorkloadFrontierObservationPosture::Exact
        }));
        assert!(long_loop.observed_lanes.iter().any(|observation| {
            observation.posture == TassadarWorkloadFrontierObservationPosture::ResearchOnly
        }));

        let sudoku = report
            .frontier_rows
            .iter()
            .find(|row| row.workload_family_id == "sudoku_class")
            .expect("sudoku row");
        assert!(sudoku.refusal_first);
        assert!(sudoku.observed_lanes.iter().any(|observation| {
            observation.posture == TassadarWorkloadFrontierObservationPosture::ResearchOnly
        }));
    }

    #[test]
    fn workload_capability_frontier_report_matches_committed_truth() {
        let generated = build_tassadar_workload_capability_frontier_report().expect("report");
        let committed: TassadarWorkloadCapabilityFrontierReport =
            load_repo_json(TASSADAR_WORKLOAD_CAPABILITY_FRONTIER_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_workload_capability_frontier_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_workload_capability_frontier_report.json");
        let written =
            write_tassadar_workload_capability_frontier_report(&output_path).expect("write report");
        let persisted: TassadarWorkloadCapabilityFrontierReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_workload_capability_frontier_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_workload_capability_frontier_report.json")
        );
    }
}
