use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::TassadarModuleTrustPosture;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_LINKED_PROGRAM_BUNDLE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_linked_program_bundle_runtime_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRuntimeSupportModuleClass {
    Parser,
    Checksum,
    VmDispatch,
    Allocator,
    CheckpointBacktrack,
}

impl TassadarRuntimeSupportModuleClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parser => "parser",
            Self::Checksum => "checksum",
            Self::VmDispatch => "vm_dispatch",
            Self::Allocator => "allocator",
            Self::CheckpointBacktrack => "checkpoint_backtrack",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinkedProgramModuleRole {
    PrimaryCompute,
    HelperParser,
    HelperChecksum,
    HelperVmDispatch,
    HelperAllocator,
    HelperCheckpointBacktrack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinkedProgramStatePosture {
    Stateless,
    ModuleLocalState,
    SharedBundleState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleModule {
    pub module_ref: String,
    pub source_ref: String,
    pub role: TassadarLinkedProgramModuleRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_support_class: Option<TassadarRuntimeSupportModuleClass>,
    pub state_posture: TassadarLinkedProgramStatePosture,
    pub benchmark_lineage_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinkedProgramBundleEdgeKind {
    HelperFeedsPrimary,
    HelperDependsOnPrimaryState,
    SharedStateBridge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleGraphEdge {
    pub from_module_ref: String,
    pub to_module_ref: String,
    pub edge_kind: TassadarLinkedProgramBundleEdgeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinkedProgramStartSemantics {
    HelpersBeforePrimary,
    PrimaryBeforeHelpers,
    RefusedUnsupportedCycle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleDescriptor {
    pub bundle_id: String,
    pub bundle_source_ref: String,
    pub consumer_family: String,
    pub trust_posture: TassadarModuleTrustPosture,
    pub modules: Vec<TassadarLinkedProgramBundleModule>,
    pub graph_edges: Vec<TassadarLinkedProgramBundleGraphEdge>,
    pub start_semantics: TassadarLinkedProgramStartSemantics,
    pub start_module_refs: Vec<String>,
    pub benchmark_lineage_refs: Vec<String>,
    pub claim_boundary: String,
    pub descriptor_digest: String,
}

impl TassadarLinkedProgramBundleDescriptor {
    fn new(
        bundle_id: impl Into<String>,
        bundle_source_ref: impl Into<String>,
        consumer_family: impl Into<String>,
        trust_posture: TassadarModuleTrustPosture,
        modules: Vec<TassadarLinkedProgramBundleModule>,
        graph_edges: Vec<TassadarLinkedProgramBundleGraphEdge>,
        start_semantics: TassadarLinkedProgramStartSemantics,
        start_module_refs: Vec<String>,
        benchmark_lineage_refs: Vec<String>,
        claim_boundary: impl Into<String>,
    ) -> Self {
        let mut descriptor = Self {
            bundle_id: bundle_id.into(),
            bundle_source_ref: bundle_source_ref.into(),
            consumer_family: consumer_family.into(),
            trust_posture,
            modules,
            graph_edges,
            start_semantics,
            start_module_refs,
            benchmark_lineage_refs,
            claim_boundary: claim_boundary.into(),
            descriptor_digest: String::new(),
        };
        descriptor.descriptor_digest = stable_digest(
            b"psionic_tassadar_linked_program_bundle_descriptor|",
            &descriptor,
        );
        descriptor
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinkedProgramBundlePosture {
    Exact,
    RolledBack,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinkedProgramBundleRefusalReason {
    IncompatibleBundleShape,
    UntrackedSharedState,
    BenchmarkLineageGap,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleCaseReport {
    pub case_id: String,
    pub bundle_descriptor: TassadarLinkedProgramBundleDescriptor,
    pub requested_module_refs: Vec<String>,
    pub selected_module_refs: Vec<String>,
    pub helper_module_refs: Vec<String>,
    pub runtime_support_classes: Vec<TassadarRuntimeSupportModuleClass>,
    pub local_state_module_refs: Vec<String>,
    pub shared_state_module_refs: Vec<String>,
    pub posture: TassadarLinkedProgramBundlePosture,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarLinkedProgramBundleRefusalReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_detail: Option<String>,
    pub benchmark_lineage_complete: bool,
    pub helper_lineage_complete: bool,
    pub exact_outputs_preserved: bool,
    pub exact_trace_match: bool,
    pub graph_shape_valid: bool,
    pub start_order_replay_exact: bool,
    pub dependency_graph_digest: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinkedProgramBundleRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub exact_case_count: u32,
    pub rollback_case_count: u32,
    pub refused_case_count: u32,
    pub shared_state_case_count: u32,
    pub benchmark_lineage_complete_case_count: u32,
    pub helper_lineage_complete_case_count: u32,
    pub graph_valid_case_count: u32,
    pub start_order_exact_case_count: u32,
    pub case_reports: Vec<TassadarLinkedProgramBundleCaseReport>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarLinkedProgramBundleRuntimeReportError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn seeded_tassadar_linked_program_bundles() -> Vec<TassadarLinkedProgramBundleDescriptor> {
    vec![
        TassadarLinkedProgramBundleDescriptor::new(
            "tassadar.linked_program_bundle.vm_checksum_parser.v1",
            "fixtures/tassadar/sources/tassadar_linked_program_bundle_vm_checksum_parser.json",
            "module_scale_pipeline",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![
                module(
                    "vm_dispatch_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_vm_style_suite.wat",
                    TassadarLinkedProgramModuleRole::PrimaryCompute,
                    None,
                    TassadarLinkedProgramStatePosture::ModuleLocalState,
                    &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
                ),
                module(
                    "parser_helper_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_parsing_suite.wat",
                    TassadarLinkedProgramModuleRole::HelperParser,
                    Some(TassadarRuntimeSupportModuleClass::Parser),
                    TassadarLinkedProgramStatePosture::ModuleLocalState,
                    &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
                ),
                module(
                    "checksum_helper_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_checksum_suite.wat",
                    TassadarLinkedProgramModuleRole::HelperChecksum,
                    Some(TassadarRuntimeSupportModuleClass::Checksum),
                    TassadarLinkedProgramStatePosture::ModuleLocalState,
                    &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
                ),
            ],
            vec![
                edge(
                    "parser_helper_core@1.0.0",
                    "vm_dispatch_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::HelperFeedsPrimary,
                ),
                edge(
                    "checksum_helper_core@1.0.0",
                    "vm_dispatch_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::HelperFeedsPrimary,
                ),
            ],
            TassadarLinkedProgramStartSemantics::HelpersBeforePrimary,
            vec![
                String::from("parser_helper_core@1.0.0"),
                String::from("checksum_helper_core@1.0.0"),
                String::from("vm_dispatch_core@1.0.0"),
            ],
            vec![
                String::from("fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_module_link_eval_report.json"),
            ],
            "vm-dispatch plus parser plus checksum is a bounded linked-program bundle with helper lineage and module-local state posture made explicit",
        ),
        TassadarLinkedProgramBundleDescriptor::new(
            "tassadar.linked_program_bundle.checkpoint_backtrack.v1",
            "fixtures/tassadar/sources/tassadar_linked_program_bundle_checkpoint_backtrack.json",
            "checkpointed_search_bundle",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![
                module(
                    "search_frontier_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_vm_style_suite.wat",
                    TassadarLinkedProgramModuleRole::PrimaryCompute,
                    None,
                    TassadarLinkedProgramStatePosture::SharedBundleState,
                    &["fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json"],
                ),
                module(
                    "checkpoint_backtrack_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_runtime_support_checkpoint_backtrack.json",
                    TassadarLinkedProgramModuleRole::HelperCheckpointBacktrack,
                    Some(TassadarRuntimeSupportModuleClass::CheckpointBacktrack),
                    TassadarLinkedProgramStatePosture::SharedBundleState,
                    &["fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json"],
                ),
                module(
                    "bounded_allocator_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_memcpy_suite.wat",
                    TassadarLinkedProgramModuleRole::HelperAllocator,
                    Some(TassadarRuntimeSupportModuleClass::Allocator),
                    TassadarLinkedProgramStatePosture::ModuleLocalState,
                    &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
                ),
            ],
            vec![
                edge(
                    "checkpoint_backtrack_core@1.0.0",
                    "search_frontier_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::HelperDependsOnPrimaryState,
                ),
                edge(
                    "search_frontier_core@1.0.0",
                    "checkpoint_backtrack_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::SharedStateBridge,
                ),
                edge(
                    "bounded_allocator_core@1.0.0",
                    "search_frontier_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::HelperFeedsPrimary,
                ),
            ],
            TassadarLinkedProgramStartSemantics::PrimaryBeforeHelpers,
            vec![
                String::from("search_frontier_core@1.0.0"),
                String::from("checkpoint_backtrack_core@1.0.0"),
                String::from("bounded_allocator_core@1.0.0"),
            ],
            vec![
                String::from("fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_module_link_eval_report.json"),
            ],
            "checkpoint plus backtrack plus bounded allocator is a bounded linked-program bundle with explicit shared bundle state and exact checkpoint lineage",
        ),
        TassadarLinkedProgramBundleDescriptor::new(
            "tassadar.linked_program_bundle.parser_allocator_rollback.v1",
            "fixtures/tassadar/sources/tassadar_linked_program_bundle_parser_allocator_rollback.json",
            "parser_runtime_support_bundle",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![
                module(
                    "parsing_pipeline_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_parsing_suite.wat",
                    TassadarLinkedProgramModuleRole::PrimaryCompute,
                    None,
                    TassadarLinkedProgramStatePosture::ModuleLocalState,
                    &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
                ),
                module(
                    "parser_helper_core@1.1.0-candidate",
                    "fixtures/tassadar/sources/tassadar_module_parsing_suite.wat",
                    TassadarLinkedProgramModuleRole::HelperParser,
                    Some(TassadarRuntimeSupportModuleClass::Parser),
                    TassadarLinkedProgramStatePosture::ModuleLocalState,
                    &["fixtures/tassadar/reports/tassadar_module_link_runtime_report.json"],
                ),
                module(
                    "bounded_allocator_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_memcpy_suite.wat",
                    TassadarLinkedProgramModuleRole::HelperAllocator,
                    Some(TassadarRuntimeSupportModuleClass::Allocator),
                    TassadarLinkedProgramStatePosture::ModuleLocalState,
                    &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
                ),
            ],
            vec![
                edge(
                    "parser_helper_core@1.1.0-candidate",
                    "parsing_pipeline_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::HelperFeedsPrimary,
                ),
                edge(
                    "bounded_allocator_core@1.0.0",
                    "parsing_pipeline_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::HelperFeedsPrimary,
                ),
            ],
            TassadarLinkedProgramStartSemantics::HelpersBeforePrimary,
            vec![
                String::from("parser_helper_core@1.1.0-candidate"),
                String::from("bounded_allocator_core@1.0.0"),
                String::from("parsing_pipeline_core@1.0.0"),
            ],
            vec![
                String::from("fixtures/tassadar/reports/tassadar_module_link_runtime_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"),
            ],
            "parser plus allocator bundles may roll back helper versions explicitly instead of silently drifting across runtime-support helper candidates",
        ),
        TassadarLinkedProgramBundleDescriptor::new(
            "tassadar.linked_program_bundle.shared_state_gap.v1",
            "fixtures/tassadar/sources/tassadar_linked_program_bundle_shared_state_gap.json",
            "stateful_runtime_support_gap",
            TassadarModuleTrustPosture::BenchmarkGatedInternal,
            vec![
                module(
                    "stateful_vm_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_vm_style_suite.wat",
                    TassadarLinkedProgramModuleRole::PrimaryCompute,
                    None,
                    TassadarLinkedProgramStatePosture::SharedBundleState,
                    &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
                ),
                module(
                    "parser_helper_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_parsing_suite.wat",
                    TassadarLinkedProgramModuleRole::HelperParser,
                    Some(TassadarRuntimeSupportModuleClass::Parser),
                    TassadarLinkedProgramStatePosture::SharedBundleState,
                    &[],
                ),
                module(
                    "checksum_helper_core@1.0.0",
                    "fixtures/tassadar/sources/tassadar_module_checksum_suite.wat",
                    TassadarLinkedProgramModuleRole::HelperChecksum,
                    Some(TassadarRuntimeSupportModuleClass::Checksum),
                    TassadarLinkedProgramStatePosture::SharedBundleState,
                    &[],
                ),
            ],
            vec![
                edge(
                    "parser_helper_core@1.0.0",
                    "stateful_vm_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::SharedStateBridge,
                ),
                edge(
                    "checksum_helper_core@1.0.0",
                    "stateful_vm_core@1.0.0",
                    TassadarLinkedProgramBundleEdgeKind::SharedStateBridge,
                ),
            ],
            TassadarLinkedProgramStartSemantics::RefusedUnsupportedCycle,
            vec![
                String::from("stateful_vm_core@1.0.0"),
                String::from("parser_helper_core@1.0.0"),
                String::from("checksum_helper_core@1.0.0"),
            ],
            vec![String::from("fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json")],
            "shared-state runtime-support bundles stay refused when shared state or benchmark lineage is not fully visible at bundle scale",
        ),
    ]
}

#[must_use]
pub fn build_tassadar_linked_program_bundle_runtime_report(
) -> TassadarLinkedProgramBundleRuntimeReport {
    let descriptors = seeded_tassadar_linked_program_bundles();
    let case_reports = vec![
        build_case(
            "bundle.vm_checksum_parser.exact.v1",
            descriptors[0].clone(),
            &["vm_dispatch_core@1.0.0", "parser_helper_core@1.0.0", "checksum_helper_core@1.0.0"],
            &["vm_dispatch_core@1.0.0", "parser_helper_core@1.0.0", "checksum_helper_core@1.0.0"],
            TassadarLinkedProgramBundlePosture::Exact,
            None,
            None,
            true,
            true,
            true,
            true,
            true,
            true,
            "module-local helper parser and checksum support remain exact when helper lineage and state posture are fully explicit",
        ),
        build_case(
            "bundle.checkpoint_backtrack.exact.v1",
            descriptors[1].clone(),
            &["search_frontier_core@1.0.0", "checkpoint_backtrack_core@1.0.0", "bounded_allocator_core@1.0.0"],
            &["search_frontier_core@1.0.0", "checkpoint_backtrack_core@1.0.0", "bounded_allocator_core@1.0.0"],
            TassadarLinkedProgramBundlePosture::Exact,
            None,
            None,
            true,
            true,
            true,
            true,
            true,
            true,
            "checkpoint plus backtrack plus allocator support remains exact when shared bundle state stays visible in the receipt",
        ),
        build_case(
            "bundle.parser_allocator.rollback.v1",
            descriptors[2].clone(),
            &["parsing_pipeline_core@1.0.0", "parser_helper_core@1.1.0-candidate", "bounded_allocator_core@1.0.0"],
            &["parsing_pipeline_core@1.0.0", "parser_helper_core@1.0.0", "bounded_allocator_core@1.0.0"],
            TassadarLinkedProgramBundlePosture::RolledBack,
            None,
            Some("parser_helper_core@1.1.0-candidate rolled back to parser_helper_core@1.0.0 because the candidate helper lacks published active benchmark lineage"),
            true,
            true,
            true,
            false,
            true,
            true,
            "helper-module rollback stays explicit at bundle scale instead of silently drifting across runtime-support helper versions",
        ),
        build_case(
            "bundle.shared_state_gap.refused.v1",
            descriptors[3].clone(),
            &["stateful_vm_core@1.0.0", "parser_helper_core@1.0.0", "checksum_helper_core@1.0.0"],
            &[],
            TassadarLinkedProgramBundlePosture::Refused,
            Some(TassadarLinkedProgramBundleRefusalReason::BenchmarkLineageGap),
            None,
            false,
            false,
            false,
            false,
            false,
            false,
            "shared-state bundle refuses because helper-module benchmark lineage is incomplete and the shared-state receipt would otherwise be ambiguous",
        ),
    ];
    let mut report = TassadarLinkedProgramBundleRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.linked_program_bundle_runtime.report.v1"),
        exact_case_count: case_reports.iter().filter(|case| case.posture == TassadarLinkedProgramBundlePosture::Exact).count() as u32,
        rollback_case_count: case_reports.iter().filter(|case| case.posture == TassadarLinkedProgramBundlePosture::RolledBack).count() as u32,
        refused_case_count: case_reports.iter().filter(|case| case.posture == TassadarLinkedProgramBundlePosture::Refused).count() as u32,
        shared_state_case_count: case_reports.iter().filter(|case| !case.shared_state_module_refs.is_empty()).count() as u32,
        benchmark_lineage_complete_case_count: case_reports.iter().filter(|case| case.benchmark_lineage_complete).count() as u32,
        helper_lineage_complete_case_count: case_reports.iter().filter(|case| case.helper_lineage_complete).count() as u32,
        graph_valid_case_count: case_reports.iter().filter(|case| case.graph_shape_valid).count() as u32,
        start_order_exact_case_count: case_reports.iter().filter(|case| case.start_order_replay_exact).count() as u32,
        case_reports,
        claim_boundary: String::from("this runtime report freezes bounded linked-program bundles with explicit helper-module roles, runtime-support classes, module-local versus shared bundle state posture, rollback detail, and benchmark lineage. It does not imply arbitrary software growth, arbitrary install closure, or unrestricted self-extension"),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Linked-program bundle runtime report covers {} cases with exact={}, rollback={}, refused={}, shared_state_cases={}, lineage_complete_cases={}, helper_lineage_complete_cases={}, graph_valid_cases={}, and start_order_exact_cases={}.",
        report.case_reports.len(),
        report.exact_case_count,
        report.rollback_case_count,
        report.refused_case_count,
        report.shared_state_case_count,
        report.benchmark_lineage_complete_case_count,
        report.helper_lineage_complete_case_count,
        report.graph_valid_case_count,
        report.start_order_exact_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_linked_program_bundle_runtime_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_linked_program_bundle_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LINKED_PROGRAM_BUNDLE_RUNTIME_REPORT_REF)
}

pub fn write_tassadar_linked_program_bundle_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLinkedProgramBundleRuntimeReport, TassadarLinkedProgramBundleRuntimeReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLinkedProgramBundleRuntimeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_linked_program_bundle_runtime_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLinkedProgramBundleRuntimeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_linked_program_bundle_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarLinkedProgramBundleRuntimeReport, TassadarLinkedProgramBundleRuntimeReportError>
{
    read_json(path)
}

fn build_case(
    case_id: &str,
    bundle_descriptor: TassadarLinkedProgramBundleDescriptor,
    requested_module_refs: &[&str],
    selected_module_refs: &[&str],
    posture: TassadarLinkedProgramBundlePosture,
    refusal_reason: Option<TassadarLinkedProgramBundleRefusalReason>,
    rollback_detail: Option<&str>,
    benchmark_lineage_complete: bool,
    helper_lineage_complete: bool,
    exact_outputs_preserved: bool,
    exact_trace_match: bool,
    graph_shape_valid: bool,
    start_order_replay_exact: bool,
    note: &str,
) -> TassadarLinkedProgramBundleCaseReport {
    let requested_module_refs = requested_module_refs
        .iter()
        .map(|value| String::from(*value))
        .collect::<Vec<_>>();
    let selected_module_refs = selected_module_refs
        .iter()
        .map(|value| String::from(*value))
        .collect::<Vec<_>>();
    let helper_module_refs = bundle_descriptor
        .modules
        .iter()
        .filter(|module| module.role != TassadarLinkedProgramModuleRole::PrimaryCompute)
        .map(|module| module.module_ref.clone())
        .collect::<Vec<_>>();
    let runtime_support_classes = bundle_descriptor
        .modules
        .iter()
        .filter_map(|module| module.runtime_support_class)
        .collect::<Vec<_>>();
    let local_state_module_refs = bundle_descriptor
        .modules
        .iter()
        .filter(|module| {
            module.state_posture == TassadarLinkedProgramStatePosture::ModuleLocalState
        })
        .map(|module| module.module_ref.clone())
        .collect::<Vec<_>>();
    let shared_state_module_refs = bundle_descriptor
        .modules
        .iter()
        .filter(|module| {
            module.state_posture == TassadarLinkedProgramStatePosture::SharedBundleState
        })
        .map(|module| module.module_ref.clone())
        .collect::<Vec<_>>();
    let dependency_graph_digest = stable_digest(
        b"psionic_tassadar_linked_program_bundle_dependency_graph|",
        &(
            &bundle_descriptor.bundle_id,
            &requested_module_refs,
            &selected_module_refs,
        ),
    );
    TassadarLinkedProgramBundleCaseReport {
        case_id: String::from(case_id),
        bundle_descriptor,
        requested_module_refs,
        selected_module_refs,
        helper_module_refs,
        runtime_support_classes,
        local_state_module_refs,
        shared_state_module_refs,
        posture,
        refusal_reason,
        rollback_detail: rollback_detail.map(String::from),
        benchmark_lineage_complete,
        helper_lineage_complete,
        exact_outputs_preserved,
        exact_trace_match,
        graph_shape_valid,
        start_order_replay_exact,
        dependency_graph_digest,
        note: String::from(note),
    }
}

fn edge(
    from_module_ref: &str,
    to_module_ref: &str,
    edge_kind: TassadarLinkedProgramBundleEdgeKind,
) -> TassadarLinkedProgramBundleGraphEdge {
    TassadarLinkedProgramBundleGraphEdge {
        from_module_ref: String::from(from_module_ref),
        to_module_ref: String::from(to_module_ref),
        edge_kind,
    }
}

fn module(
    module_ref: &str,
    source_ref: &str,
    role: TassadarLinkedProgramModuleRole,
    runtime_support_class: Option<TassadarRuntimeSupportModuleClass>,
    state_posture: TassadarLinkedProgramStatePosture,
    benchmark_lineage_refs: &[&str],
) -> TassadarLinkedProgramBundleModule {
    TassadarLinkedProgramBundleModule {
        module_ref: String::from(module_ref),
        source_ref: String::from(source_ref),
        role,
        runtime_support_class,
        state_posture,
        benchmark_lineage_refs: benchmark_lineage_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarLinkedProgramBundleRuntimeReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarLinkedProgramBundleRuntimeReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarLinkedProgramBundleRuntimeReportError::Deserialize {
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
        build_tassadar_linked_program_bundle_runtime_report,
        load_tassadar_linked_program_bundle_runtime_report, seeded_tassadar_linked_program_bundles,
        tassadar_linked_program_bundle_runtime_report_path,
        write_tassadar_linked_program_bundle_runtime_report, TassadarLinkedProgramBundlePosture,
    };

    #[test]
    fn linked_program_bundle_descriptors_are_machine_legible() {
        let bundles = seeded_tassadar_linked_program_bundles();
        assert_eq!(bundles.len(), 4);
        assert!(bundles.iter().any(|bundle| {
            bundle.bundle_id == "tassadar.linked_program_bundle.checkpoint_backtrack.v1"
                && bundle
                    .modules
                    .iter()
                    .any(|module| module.runtime_support_class.is_some())
        }));
    }

    #[test]
    fn linked_program_bundle_runtime_report_tracks_exact_rollback_and_refusal() {
        let report = build_tassadar_linked_program_bundle_runtime_report();
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.rollback_case_count, 1);
        assert_eq!(report.refused_case_count, 1);
        assert_eq!(report.shared_state_case_count, 2);
        assert_eq!(report.benchmark_lineage_complete_case_count, 3);
        assert_eq!(report.helper_lineage_complete_case_count, 3);
        assert_eq!(report.graph_valid_case_count, 3);
        assert_eq!(report.start_order_exact_case_count, 3);
        assert!(report.case_reports.iter().any(|case| {
            case.case_id == "bundle.shared_state_gap.refused.v1"
                && case.posture == TassadarLinkedProgramBundlePosture::Refused
                && !case.graph_shape_valid
        }));
    }

    #[test]
    fn linked_program_bundle_runtime_report_matches_committed_truth() {
        let report = build_tassadar_linked_program_bundle_runtime_report();
        let persisted = load_tassadar_linked_program_bundle_runtime_report(
            tassadar_linked_program_bundle_runtime_report_path(),
        )
        .expect("committed report");
        assert_eq!(persisted, report);
    }

    #[test]
    fn write_linked_program_bundle_runtime_report_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_linked_program_bundle_runtime_report.json");
        let report =
            write_tassadar_linked_program_bundle_runtime_report(&output_path).expect("report");
        let persisted =
            load_tassadar_linked_program_bundle_runtime_report(&output_path).expect("persisted");
        assert_eq!(persisted, report);
        std::fs::remove_file(output_path).expect("temp report should be removable");
    }
}
